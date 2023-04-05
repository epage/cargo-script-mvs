#![forbid(unsafe_code)]

mod arguments;
mod build_kind;
mod dirs;
mod manifest;
mod templates;
mod util;

#[cfg(windows)]
mod file_assoc;

#[cfg(not(windows))]
mod file_assoc {}

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha1::{Digest, Sha1};

use crate::build_kind::BuildKind;
use crate::util::Defer;
use arguments::Args;

fn main() {
    env_logger::init();

    match try_main() {
        Ok(code) => {
            std::process::exit(code);
        }
        Err(ref err) => {
            let stderr = &mut std::io::stderr();
            let _ = writeln!(stderr, "error: {err}");
            std::process::exit(1);
        }
    }
}

fn try_main() -> anyhow::Result<i32> {
    let args = arguments::Args::parse()?;
    log::trace!("Arguments: {:?}", args);

    #[cfg(windows)]
    {
        if args.install_file_association {
            file_assoc::install_file_association()?;
            return Ok(0);
        } else if args.uninstall_file_association {
            file_assoc::uninstall_file_association()?;
            return Ok(0);
        }
    }

    if args.clear_cache {
        clean_cache()?;
        if args.script.is_none() {
            println!("rust-script cache cleared.");
            return Ok(0);
        }
    }

    let input = match (args.script.clone().unwrap(), args.expr) {
        (script, false) => {
            let (path, mut file) = find_script(&script).ok_or_else(|| {
                anyhow::format_err!("could not find script: {}", script.to_string_lossy())
            })?;

            let script_name = path
                .file_stem()
                .map(|os| os.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());

            let mut body = String::new();
            file.read_to_string(&mut body)?;

            let script_path = std::env::current_dir()?.join(path);

            Input::File(script_name, script_path, body)
        }
        (expr, true) => {
            let expr = expr
                .to_str()
                .ok_or_else(|| {
                    anyhow::format_err!("expr must be UTF-8, got {}", expr.to_string_lossy())
                })?
                .to_owned();
            Input::Expr(expr)
        }
    };
    log::trace!("input: {:?}", input);

    // Setup environment variables early so it's available at compilation time of scripts,
    // to allow e.g. include!(concat!(env!("RUST_SCRIPT_BASE_PATH"), "/script-module.rs"));
    std::env::set_var(
        "RUST_SCRIPT_PATH",
        input.path().unwrap_or_else(|| Path::new("")),
    );
    std::env::set_var("RUST_SCRIPT_SAFE_NAME", input.safe_name());
    std::env::set_var("RUST_SCRIPT_PKG_NAME", input.package_name());
    std::env::set_var("RUST_SCRIPT_BASE_PATH", input.base_path());

    let action = decide_action_for(&input, &args)?;
    log::trace!("action: {:?}", action);

    gen_pkg_and_compile(&input, &action)?;

    // Once we're done, clean out old packages from the cache.
    // There's no point if we've already done a full clear, though.
    let _defer_clear = {
        // To get around partially moved args problems.
        let cc = args.clear_cache;
        Defer::<_, anyhow::Error>::new(move || {
            if !cc {
                gc_cache(MAX_CACHE_AGE)?;
            }
            Ok(())
        })
    };

    let mut cmd = action.cargo(&args.script_args)?;

    #[cfg(unix)]
    {
        let err = cmd.exec();
        Err(err.into())
    }
    #[cfg(not(unix))]
    {
        let exit_code = cmd.status().map(|st| st.code().unwrap_or(1))?;
        Ok(exit_code)
    }
}

/// How old can stuff in the cache be before we automatically clear it out?
pub const MAX_CACHE_AGE: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// Empty the cache
fn clean_cache() -> anyhow::Result<()> {
    log::info!("cleaning cache");

    let cache_dir = dirs::binary_cache_path()?;
    if cache_dir.exists() {
        if let Err(err) = fs::remove_dir_all(&cache_dir) {
            log::error!("failed to remove binary cache {:?}: {}", cache_dir, err);
        }
    }
    Ok(())
}

/// Clear old entries
///
/// Looks for all folders whose metadata says they were created at least `max_age` in the past and
/// kills them dead.
fn gc_cache(max_age: std::time::Duration) -> anyhow::Result<()> {
    log::info!("cleaning cache with max_age: {:?}", max_age);

    let cutoff = std::time::SystemTime::now() - max_age;
    log::trace!("cutoff:     {:>20?} ms", cutoff);

    let cache_dir = dirs::generated_projects_cache_path()?;
    if cache_dir.exists() {
        for child in fs::read_dir(cache_dir)? {
            let child = child?;
            let path = child.path();
            if path.is_file() {
                continue;
            }

            log::trace!("checking: {:?}", path);

            let remove_dir = || {
                let meta_mtime = child.metadata().and_then(|m| m.modified()).ok();
                log::trace!("meta_mtime: {:>20?} ms", meta_mtime);

                if let Some(meta_mtime) = meta_mtime {
                    meta_mtime <= cutoff
                } else {
                    true
                }
            };

            if remove_dir() {
                log::debug!("removing {:?}", path);
                if let Err(err) = fs::remove_dir_all(&path) {
                    log::error!("failed to remove {:?} from cache: {}", path, err);
                }
            }
        }
    }

    log::trace!("done cleaning cache.");
    Ok(())
}

/// Generate and compile a package from the input.
fn gen_pkg_and_compile(input: &Input, action: &InputAction) -> anyhow::Result<()> {
    let pkg_path = &action.pkg_path;

    let mani_str = &action.manifest;
    let script_str = &action.script;

    log::trace!("creating pkg dir...");
    fs::create_dir_all(pkg_path)?;
    let cleanup_dir: Defer<_, anyhow::Error> = Defer::new(|| {
        // DO NOT try deleting ANYTHING if we're not cleaning up inside our own cache.  We *DO NOT* want to risk killing user files.
        if action.using_cache {
            log::debug!("cleaning up cache directory {}", pkg_path.display());
            fs::remove_dir_all(pkg_path)?;
        }
        Ok(())
    });

    log::trace!("generating Cargo package...");
    let mani_path = action.manifest_path();
    let script_path = pkg_path.join(format!("{}.rs", input.safe_name()));

    overwrite_file(&mani_path, mani_str)?;
    overwrite_file(&script_path, script_str)?;

    log::trace!("disarming pkg dir cleanup...");
    cleanup_dir.disarm();

    Ok(())
}

/// This represents what to do with the input provided by the user.
#[derive(Debug)]
struct InputAction {
    /// Always show cargo output?
    cargo_output: bool,

    /// Force Cargo to do a recompile, even if it thinks it doesn't have to.
    ///
    /// `compile` must be `true` for this to have any effect.
    force_compile: bool,

    /// Directory where the package should live.
    pkg_path: PathBuf,

    /// Is the package directory in the cache?
    ///
    /// Currently, this can be inferred from `emit_metadata`, but there's no *intrinsic* reason they should be tied together.
    using_cache: bool,

    /// Which toolchain the script should be built with.
    ///
    /// `None` indicates that the script should be built with a stable toolchain.
    toolchain_version: Option<String>,

    /// The package metadata structure for the current invocation.
    metadata: PackageMetadata,

    /// The package manifest contents.
    manifest: String,

    /// The script source.
    script: String,

    /// Did the user ask to run tests or benchmarks?
    build_kind: BuildKind,

    // Name of the built binary
    bin_name: String,

    /// The file stem for the script source
    safe_name: String,
}

impl InputAction {
    fn manifest_path(&self) -> PathBuf {
        self.pkg_path.join("Cargo.toml")
    }

    fn script_path(&self) -> PathBuf {
        self.pkg_path.join(format!("{}.rs", self.safe_name))
    }

    fn cargo(&self, script_args: &[OsString]) -> anyhow::Result<Command> {
        let release_mode = !self.metadata.debug && !matches!(self.build_kind, BuildKind::Bench);

        let built_binary_path = dirs::binary_cache_path()?
            .join(if release_mode { "release" } else { "debug" })
            .join(&format!(
                "{}{}",
                self.bin_name,
                std::env::consts::EXE_SUFFIX
            ));

        let manifest_path = self.manifest_path();

        let execute_command = || {
            let mut cmd = Command::new(&built_binary_path);
            cmd.args(script_args.iter());
            // Set tracing on if not set
            if std::env::var_os("RUST_BACKTRACE").is_none() {
                cmd.env("RUST_BACKTRACE", "1");
                log::trace!("setting RUST_BACKTRACE=1 for this cargo run");
            }

            cmd
        };

        if matches!(self.build_kind, BuildKind::Normal) && !self.force_compile {
            let script_path = self.script_path();

            match fs::File::open(&built_binary_path) {
                Ok(built_binary_file) => {
                    let built_binary_mtime =
                        built_binary_file.metadata().unwrap().modified().unwrap();
                    let script_mtime = script_path.metadata()?.modified()?;
                    let manifest_mtime = manifest_path.metadata()?.modified()?;
                    log::trace!(
                        "binary {}: {:?}",
                        built_binary_path.display(),
                        built_binary_mtime
                    );
                    log::trace!("main {}: {:?}", script_path.display(), script_mtime);
                    log::trace!(
                        "manifest: {}: {:?}",
                        manifest_path.display(),
                        manifest_mtime
                    );
                    if built_binary_mtime.cmp(&script_mtime).is_ge()
                        && built_binary_mtime.cmp(&manifest_mtime).is_ge()
                    {
                        return Ok(execute_command());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Continue
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        let mut cmd = if let Some(toolchain_version) = self.toolchain_version.as_deref() {
            if std::env::var_os("RUSTUP_TOOLCHAIN").is_some() {
                // Running inside rustup which can't always call into rustup proxies, so explicitly call
                // rustup
                let mut cmd = Command::new("rustup");
                cmd.args(["run", toolchain_version, "cargo"]);
                cmd
            } else {
                let mut cmd = Command::new("cargo");
                cmd.arg(format!("+{toolchain_version}"));
                cmd
            }
        } else {
            Command::new("cargo")
        };

        cmd.arg(self.build_kind.exec_command());

        if matches!(self.build_kind, BuildKind::Normal) && !self.cargo_output {
            cmd.arg("-q");
        }

        cmd.current_dir(&self.pkg_path);

        if force_cargo_color() {
            cmd.arg("--color").arg("always");
        }

        cmd.arg("--target-dir");
        cmd.arg(dirs::binary_cache_path()?);

        if release_mode {
            cmd.arg("--release");
        }

        if matches!(self.build_kind, BuildKind::Normal) {
            if cmd.status()?.success() {
                cmd = execute_command();
            } else {
                anyhow::bail!("compilation failed")
            }
        }

        Ok(cmd)
    }
}

/// The metadata here serves two purposes:
///
/// 1. It records everything necessary for compilation and execution of a package.
/// 2. It records everything that must be exactly the same in order for a cached executable to still be valid, in addition to the content hash.
#[derive(Clone, Debug)]
struct PackageMetadata {
    /// Was the script compiled in debug mode?
    debug: bool,
}

/// For the given input, this constructs the package metadata and checks the cache to see what should be done.
fn decide_action_for(input: &Input, args: &Args) -> anyhow::Result<InputAction> {
    let input_id = input.compute_id();
    log::trace!("id: {:?}", input_id);

    let (pkg_path, using_cache) = if let Some(pkg_path) = args.pkg_path.as_deref() {
        (pkg_path.to_owned(), false)
    } else {
        let cache_path = dirs::generated_projects_cache_path()?;
        (cache_path.join(&input_id), true)
    };
    log::trace!("pkg_path: {}", pkg_path.display());
    log::trace!("using_cache: {}", using_cache);

    let pkg_name = input.package_name();
    let bin_name = format!("{}_{}", &*pkg_name, input_id.to_str().unwrap());

    let (mani_str, script_str) = manifest::split_input(input, &bin_name)?;

    // Forcibly override some flags based on build kind.
    let (debug, force) = match args.build_kind {
        BuildKind::Normal => (args.debug, args.force),
        BuildKind::Test => (true, false),
        BuildKind::Bench => (false, false),
    };

    let input_meta = { PackageMetadata { debug } };
    log::trace!("input_meta: {:?}", input_meta);

    let toolchain_version = args
        .toolchain_version
        .clone()
        .or_else(|| match args.build_kind {
            BuildKind::Bench => Some("nightly".into()),
            _ => None,
        });

    let mut action = InputAction {
        cargo_output: args.cargo_output,
        force_compile: force,
        pkg_path,
        using_cache,
        toolchain_version,
        metadata: input_meta,
        manifest: mani_str,
        script: script_str,
        build_kind: args.build_kind,
        bin_name,
        safe_name: input.safe_name().to_owned(),
    };

    // If we're not doing a regular build, stop.
    match action.build_kind {
        BuildKind::Normal => (),
        BuildKind::Test | BuildKind::Bench => {
            log::debug!("not recompiling because: user asked for test/bench");
            action.force_compile = false;
            return Ok(action);
        }
    }

    Ok(action)
}

/// Attempts to locate the script specified by the given path.
fn find_script<P>(path: P) -> Option<(PathBuf, fs::File)>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    if let Ok(file) = fs::File::open(path) {
        return Some((path.into(), file));
    }

    if path.extension().is_none() {
        for ext in ["ers", "rs"] {
            let path = path.with_extension(ext);
            if let Ok(file) = fs::File::open(&path) {
                return Some((path, file));
            }
        }
    }

    None
}

/// Represents an input source for a script.
#[derive(Clone, Debug)]
pub enum Input {
    /// The input is a script file.
    ///
    /// The tuple members are: the name, absolute path, script contents, last modified time.
    File(String, PathBuf, String),

    /// The input is an expression.
    ///
    /// The tuple member is: the script contents, and the template (if any).
    Expr(String),
}

impl Input {
    /// Return the path to the script, if it has one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File(_, path, _) => Some(path.as_path()),
            Self::Expr(..) => None,
        }
    }

    /// Return the "safe name" for the input.  This should be filename-safe.
    ///
    /// Currently, nothing is done to ensure this, other than hoping *really hard* that we don't get fed some excessively bizarre input filename.
    pub fn safe_name(&self) -> &str {
        match self {
            Self::File(name, _, _) => name,
            Self::Expr(..) => "expr",
        }
    }

    /// Return the package name for the input.  This should be a valid Rust identifier.
    pub fn package_name(&self) -> String {
        let name = self.safe_name();
        let mut r = String::with_capacity(name.len());

        for (i, c) in name.chars().enumerate() {
            match (i, c) {
                (0, '0'..='9') => {
                    r.push('_');
                    r.push(c);
                }
                (_, '0'..='9') | (_, 'a'..='z') | (_, '_') | (_, '-') => {
                    r.push(c);
                }
                (_, 'A'..='Z') => {
                    // Convert uppercase characters to lowercase to avoid `non_snake_case` warnings.
                    r.push(c.to_ascii_lowercase());
                }
                (_, _) => {
                    r.push('_');
                }
            }
        }

        r
    }

    /// Base directory for resolving relative paths.
    pub fn base_path(&self) -> PathBuf {
        match self {
            Self::File(_, path, _) => path
                .parent()
                .expect("couldn't get parent directory for file input base path")
                .into(),
            Self::Expr(..) => {
                std::env::current_dir().expect("couldn't get current directory for input base path")
            }
        }
    }

    // Compute the package ID for the input.
    // This is used as the name of the cache folder into which the Cargo package
    // will be generated.
    pub fn compute_id(&self) -> OsString {
        match self {
            Self::File(_, path, _) => {
                let mut hasher = Sha1::new();

                // Hash the path to the script.
                hasher.update(&*path.to_string_lossy());
                let mut digest = format!("{:x}", hasher.finalize());
                digest.truncate(ID_DIGEST_LEN_MAX);

                let mut id = OsString::new();
                id.push(&*digest);
                id
            }
            Self::Expr(content) => {
                let mut hasher = Sha1::new();

                hasher.update(content);
                let mut digest = format!("{:x}", hasher.finalize());
                digest.truncate(ID_DIGEST_LEN_MAX);

                let mut id = OsString::new();
                id.push(&*digest);
                id
            }
        }
    }
}

/// When generating a package's unique ID, how many hex nibbles of the digest should be used *at most*?
///
/// The largest meaningful value is `40`.
pub const ID_DIGEST_LEN_MAX: usize = 24;

/// Overwrite a file if and only if the contents have changed.
fn overwrite_file(path: &std::path::Path, content: &str) -> anyhow::Result<()> {
    log::trace!("overwrite_file({:?}, _)", path);
    let mut existing_content = String::new();
    match fs::File::open(path) {
        Ok(mut file) => {
            file.read_to_string(&mut existing_content)?;
            if existing_content == content {
                log::trace!("Equal content");
                return Ok(());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Continue
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    log::trace!(".. files differ");
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::format_err!("The given path should be a file"))?;
    let mut temp_file = tempfile::NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    temp_file.persist(path)?;
    Ok(())
}

/// Returns `true` if `rust-script` should force Cargo to use coloured output.
///
/// This depends on whether `rust-script`'s STDERR is connected to a TTY or not.
pub fn force_cargo_color() -> bool {
    #[cfg(unix)]
    {
        atty::is(atty::Stream::Stderr)
    }
    #[cfg(windows)]
    {
        false
    }
}

#[test]
fn test_package_name() {
    let input = Input::File("Script".into(), Path::new("path").into(), "script".into());
    assert_eq!("script", input.package_name());
    let input = Input::File("1Script".into(), Path::new("path").into(), "script".into());
    assert_eq!("_1script", input.package_name());
}
