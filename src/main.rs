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
use std::io::{BufWriter, Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
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

    let run_quietly = !action.cargo_output;
    let mut cmd = action.cargo(action.build_kind, &args.script_args, run_quietly)?;

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
                // Ok, so *why* aren't we using `modified in the package metadata?
                // The point of *that* is to track what we know about the input.
                // The problem here is that `--expr` and `--loop` don't *have*
                // modification times; they just *are*.
                // Now, `PackageMetadata` *could* be modified to store, say, the
                // moment in time the input was compiled, but then we couldn't use
                // that field for metadata matching when decided whether or not a
                // *file* input should be recompiled.
                // So, instead, we're just going to go by the timestamp on the
                // metadata file *itself*.
                let meta_mtime = {
                    let meta_path = get_pkg_metadata_path(&path);
                    let meta_file = match fs::File::open(meta_path) {
                        Ok(file) => file,
                        Err(..) => {
                            log::trace!("couldn't open metadata for {:?}", path);
                            return true;
                        }
                    };
                    meta_file.metadata().and_then(|m| m.modified()).ok()
                };
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
///
/// Why take `PackageMetadata`?  To ensure that any information we need to depend on for compilation *first* passes through `decide_action_for` *and* is less likely to not be serialised with the rest of the metadata.
fn gen_pkg_and_compile(input: &Input, action: &InputAction) -> anyhow::Result<()> {
    let pkg_path = &action.pkg_path;
    let meta = &action.metadata;
    let old_meta = action.old_metadata.as_ref();

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

    let mut meta = meta.clone();

    log::trace!("generating Cargo package...");
    let mani_path = action.manifest_path();
    let mani_hash = old_meta.map(|m| &*m.manifest_hash);
    match overwrite_file(mani_path, mani_str, mani_hash)? {
        FileOverwrite::Same => (),
        FileOverwrite::Changed { new_hash } => {
            meta.manifest_hash = new_hash;
        }
    }

    {
        let script_path = pkg_path.join(format!("{}.rs", input.safe_name()));
        // There are times (particularly involving shared target dirs) where we can't rely
        // on Cargo to correctly detect invalidated builds. As such, if we've been told to
        // *force* a recompile, we'll deliberately force the script to be overwritten,
        // which will invalidate the timestamp, which will lead to a recompile.
        let script_hash = if action.force_compile {
            log::debug!("told to force compile, ignoring script hash");
            None
        } else {
            old_meta.map(|m| &*m.script_hash)
        };
        match overwrite_file(script_path, script_str, script_hash)? {
            FileOverwrite::Same => (),
            FileOverwrite::Changed { new_hash } => {
                meta.script_hash = new_hash;
            }
        }
    }

    let meta = meta;

    // Write out metadata *now*.  Remember that we check the timestamp on the metadata, *not* on the executable.
    if action.emit_metadata {
        log::trace!("emitting metadata...");
        write_pkg_metadata(pkg_path, &meta)?;
    }

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

    /// Emit a metadata file?
    emit_metadata: bool,

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

    /// The package metadata structure for the *previous* invocation, if it exists.
    old_metadata: Option<PackageMetadata>,

    /// The package manifest contents.
    manifest: String,

    /// The script source.
    script: String,

    /// Did the user ask to run tests or benchmarks?
    build_kind: BuildKind,
}

impl InputAction {
    fn manifest_path(&self) -> PathBuf {
        self.pkg_path.join("Cargo.toml")
    }

    fn cargo(
        &self,
        build_kind: BuildKind,
        script_args: &[OsString],
        run_quietly: bool,
    ) -> anyhow::Result<Command> {
        cargo(
            build_kind,
            &self.manifest_path().to_string_lossy(),
            self.toolchain_version.as_deref(),
            &self.metadata,
            script_args,
            run_quietly,
        )
    }
}

/// The metadata here serves two purposes:
///
/// 1. It records everything necessary for compilation and execution of a package.
/// 2. It records everything that must be exactly the same in order for a cached executable to still be valid, in addition to the content hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageMetadata {
    /// Path to the script file.
    path: Option<String>,

    /// Was the script compiled in debug mode?
    debug: bool,

    /// Cargo features
    features: Option<String>,

    /// Hash of the generated `Cargo.toml` file.
    manifest_hash: String,

    /// Hash of the generated source file.
    script_hash: String,
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

    let (mani_str, script_str) = manifest::split_input(input, &input_id)?;

    // Forcibly override some flags based on build kind.
    let (debug, force) = match args.build_kind {
        BuildKind::Normal => (args.debug, args.force),
        BuildKind::Test => (true, false),
        BuildKind::Bench => (false, false),
    };

    let input_meta = {
        let path = match input {
            Input::File(_, path, _) => Some(path.to_string_lossy().into_owned()),
            _ => None,
        };
        let features = if args.features.is_empty() {
            None
        } else {
            Some(args.features.join(" "))
        };
        PackageMetadata {
            path,
            debug,
            features,
            manifest_hash: hash_str(&mani_str),
            script_hash: hash_str(&script_str),
        }
    };
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
        emit_metadata: true,
        pkg_path,
        using_cache,
        toolchain_version,
        metadata: input_meta,
        old_metadata: None,
        manifest: mani_str,
        script: script_str,
        build_kind: args.build_kind,
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

    action.old_metadata = match get_pkg_metadata(&action.pkg_path) {
        Ok(meta) => Some(meta),
        Err(err) => {
            log::debug!(
                "recompiling since failed to load metadata: {}",
                err.to_string()
            );
            None
        }
    };

    Ok(action)
}

/// Load the package metadata, given the path to the package's cache folder.
fn get_pkg_metadata<P>(pkg_path: P) -> anyhow::Result<PackageMetadata>
where
    P: AsRef<Path>,
{
    let meta_path = get_pkg_metadata_path(pkg_path);
    log::trace!("meta_path: {:?}", meta_path);
    let mut meta_file = fs::File::open(&meta_path)?;

    let meta_str = {
        let mut s = String::new();
        meta_file.read_to_string(&mut s).unwrap();
        s
    };
    let meta: PackageMetadata = serde_json::from_str(&meta_str)?;

    Ok(meta)
}

/// Work out the path to a package's metadata file.
fn get_pkg_metadata_path<P>(pkg_path: P) -> PathBuf
where
    P: AsRef<Path>,
{
    pkg_path.as_ref().join("metadata.json")
}

/// Save the package metadata, given the path to the package's cache folder.
fn write_pkg_metadata<P>(pkg_path: P, meta: &PackageMetadata) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let meta_path = get_pkg_metadata_path(&pkg_path);
    log::trace!("meta_path: {:?}", meta_path);
    let mut temp_file = tempfile::NamedTempFile::new_in(&pkg_path)?;
    serde_json::to_writer(BufWriter::new(&temp_file), meta)?;
    temp_file.flush()?;
    temp_file.persist(&meta_path)?;
    Ok(())
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

/// Shorthand for hashing a string.
fn hash_str(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s);
    format!("{:x}", hasher.finalize())
}

enum FileOverwrite {
    Same,
    Changed { new_hash: String },
}

/// Overwrite a file if and only if the contents have changed.
fn overwrite_file<P>(path: P, content: &str, hash: Option<&str>) -> anyhow::Result<FileOverwrite>
where
    P: AsRef<Path>,
{
    log::trace!("overwrite_file({:?}, _, {:?})", path.as_ref(), hash);
    let new_hash = hash_str(content);
    if Some(&*new_hash) == hash {
        log::trace!(".. hashes match");
        return Ok(FileOverwrite::Same);
    }

    log::trace!(".. hashes differ; new_hash: {:?}", new_hash);
    let dir = path
        .as_ref()
        .parent()
        .ok_or_else(|| anyhow::format_err!("The given path should be a file"))?;
    let mut temp_file = tempfile::NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    temp_file.persist(path)?;
    Ok(FileOverwrite::Changed { new_hash })
}

/// Constructs a Cargo command that runs on the script package.
fn cargo(
    build_kind: BuildKind,
    manifest: &str,
    toolchain_version: Option<&str>,
    meta: &PackageMetadata,
    script_args: &[OsString],
    run_quietly: bool,
) -> anyhow::Result<Command> {
    // Always specify a toolchain to avoid being affected by rust-version(.toml) files:
    let toolchain_version = toolchain_version.unwrap_or("stable");

    let mut cmd = if std::env::var_os("RUSTUP_TOOLCHAIN").is_some() {
        // Running inside rustup which can't always call into rustup proxies, so explicitly call
        // rustup
        let mut cmd = Command::new("rustup");
        cmd.args(["run", toolchain_version, "cargo"]);
        cmd
    } else {
        let mut cmd = Command::new("cargo");
        cmd.arg(format!("+{toolchain_version}"));
        cmd
    };

    // Set tracing on if not set
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        cmd.env("RUST_BACKTRACE", "1");
        log::trace!("setting RUST_BACKTRACE=1 for this cargo run");
    }

    cmd.arg(build_kind.exec_command());

    if matches!(build_kind, BuildKind::Normal) && run_quietly {
        cmd.arg("-q");
    }

    cmd.arg("--manifest-path").arg(manifest);

    if force_cargo_color() {
        cmd.arg("--color").arg("always");
    }

    let cargo_target_dir = format!("{}", dirs::binary_cache_path()?.display(),);
    cmd.arg("--target-dir");
    cmd.arg(cargo_target_dir);

    // Block `--release` on `bench`.
    if !meta.debug && !matches!(build_kind, BuildKind::Bench) {
        cmd.arg("--release");
    }

    if let Some(ref features) = meta.features {
        cmd.arg("--features").arg(features);
    }

    if matches!(build_kind, BuildKind::Normal) && !script_args.is_empty() {
        cmd.arg("--");
        cmd.args(script_args.iter());
    }

    Ok(cmd)
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
