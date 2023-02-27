#![forbid(unsafe_code)]

/// If this is set to `false`, then code that automatically deletes stuff *won't*.
const ALLOW_AUTO_REMOVE: bool = true;

mod error;
mod manifest;
mod platform;
mod templates;
mod util;

#[cfg(windows)]
mod file_assoc;

#[cfg(not(windows))]
mod file_assoc {}

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{MainError, MainResult};
use crate::util::Defer;
use sha1::{Digest, Sha1};

#[derive(Debug)]
struct Args {
    script: Option<OsString>,
    script_args: Vec<OsString>,
    features: Vec<String>,

    expr: bool,

    pkg_path: Option<PathBuf>,
    cargo_output: bool,
    clear_cache: bool,
    debug: bool,
    force: bool,
    build_kind: BuildKind,
    template: Option<String>,
    list_templates: bool,
    // This is a String instead of an enum since one can have custom
    // toolchains (ex. a rustc developer will probably have `stage1`):
    toolchain_version: Option<String>,

    #[cfg(windows)]
    install_file_association: bool,
    #[cfg(windows)]
    uninstall_file_association: bool,

    #[allow(dead_code)]
    unstable_flags: Vec<UnstableFlags>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
enum UnstableFlags {
    Test,
    Bench,
    ToolchainVersion,
    Expr,
}

#[derive(Copy, Clone, Debug)]
enum BuildKind {
    Normal,
    Test,
    Bench,
}

impl BuildKind {
    fn exec_command(&self) -> &'static str {
        match *self {
            Self::Normal => "run",
            Self::Test => "test",
            Self::Bench => "bench",
        }
    }

    fn from_flags(test: bool, bench: bool) -> Self {
        match (test, bench) {
            (false, false) => Self::Normal,
            (true, false) => Self::Test,
            (false, true) => Self::Bench,
            _ => panic!("got both test and bench"),
        }
    }
}

fn parse_args() -> MainResult<Args> {
    use clap::{Arg, Command};
    let about = r#"Compiles and runs a Rust script"#;

    let app = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
        .arg(
            Arg::new("script")
                .help("Script file or expression to execute")
                .value_parser(clap::value_parser!(OsString))
                .required_unless_present_any(if cfg!(windows) {
                    vec![
                        "clear-cache",
                        "list-templates",
                        "install-file-association",
                        "uninstall-file-association",
                    ]
                } else {
                    vec!["clear-cache", "list-templates"]
                })
                .conflicts_with_all(if cfg!(windows) {
                    vec![
                        "list-templates",
                        "install-file-association",
                        "uninstall-file-association",
                    ]
                } else {
                    vec!["list-templates"]
                })
                .num_args(1..)
                .trailing_var_arg(true),
        )
        .arg(
            Arg::new("expr")
                .help("Execute <script> as a literal expression and display the result (unstable)")
                .long("expr")
                .short('e')
                .action(clap::ArgAction::SetTrue)
                .requires("script"),
        )
        // Options that impact the script being executed.
        .arg(
            Arg::new("cargo-output")
                .help("Show output from cargo when building")
                .short('o')
                .long("cargo-output")
                .action(clap::ArgAction::SetTrue)
                .requires("script"),
        )
        // Set the default debug variable to false
        .arg(
            Arg::new("release")
                .help("Build a release executable, an optimised one")
                .short('r')
                .long("release")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with_all(["bench"]),
        )
        .arg(
            Arg::new("features")
                .help("Cargo features to pass when building and running")
                .short('F')
                .long("features")
                .action(clap::ArgAction::Append),
        )
        // Options that change how rust-script itself behaves, and don't alter what the script will do.
        .arg(
            Arg::new("clear-cache")
                .help("Clears out the script cache")
                .long("clear-cache")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .help("Force the script to be rebuilt")
                .long("force")
                .action(clap::ArgAction::SetTrue)
                .requires("script"),
        )
        .arg(
            Arg::new("pkg_path")
                .help("Specify where to place the generated Cargo package")
                .long("pkg-path")
                .value_parser(clap::value_parser!(PathBuf))
                .requires("script")
                .conflicts_with_all(["clear-cache", "force"]),
        )
        .arg(
            Arg::new("test")
                .help("Compile and run tests (unstable)")
                .long("test")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with_all(["bench", "force"]),
        )
        .arg(
            Arg::new("bench")
                .help("Compile and run benchmarks. Requires a nightly toolchain (unstable)")
                .long("bench")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with_all(["test", "force"]),
        )
        .arg(
            Arg::new("template")
                .help("Specify a template to use for expression scripts")
                .long("template")
                .short('t')
                .requires("expr"),
        )
        .arg(
            Arg::new("toolchain-version")
                .help("Build the script using the given toolchain version (unstable)")
                .long("toolchain-version")
                // "channel"
                .short('c')
                // FIXME: remove if benchmarking is stabilized
                .conflicts_with("bench"),
        )
        .arg(
            Arg::new("list-templates")
                .help("List the available templates")
                .long("list-templates")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("unstable_flags")
                .help("Unstable (nightly-only) flags")
                .short('Z')
                .value_name("FLAG")
                .global(true)
                .value_parser(clap::value_parser!(UnstableFlags))
                .action(clap::ArgAction::Append),
        );

    #[cfg(windows)]
    let app = app
        .arg(
            Arg::new("install-file-association")
                .help("Install a file association so that rust-script executes .ers files")
                .long("install-file-association")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("uninstall-file-association")
                .help("Uninstall the file association that makes rust-script execute .ers files")
                .long("uninstall-file-association")
                .action(clap::ArgAction::SetTrue),
        )
        .group(
            clap::ArgGroup::new("file-association")
                .args(&["install-file-association", "uninstall-file-association"]),
        );

    let m = app.get_matches();

    let unstable_flags = m
        .get_many::<UnstableFlags>("unstable_flags")
        .unwrap_or_default()
        .copied()
        .collect::<Vec<_>>();

    let script_and_args = m
        .get_many::<OsString>("script")
        .map(|o| o.map(|s| s.to_owned()));
    let script;
    let script_args: Vec<OsString>;
    if let Some(mut script_and_args) = script_and_args {
        script = script_and_args.next();
        script_args = script_and_args.collect();
    } else {
        script = None;
        script_args = Vec::new();
    }

    let build_kind = BuildKind::from_flags(
        *m.get_one::<bool>("test").expect("defaulted"),
        *m.get_one::<bool>("bench").expect("defaulted"),
    );
    match build_kind {
        BuildKind::Normal => {}
        BuildKind::Test => {
            if !unstable_flags.contains(&UnstableFlags::Test) {
                return Err(
                    "`--test` is unstable and requires `-Z test` (epage/cargo-script-mvs#29)."
                        .into(),
                );
            }
        }
        BuildKind::Bench => {
            if !unstable_flags.contains(&UnstableFlags::Bench) {
                return Err(
                    "`--bench` is unstable and requires `-Z bench` (epage/cargo-script-mvs#68)."
                        .into(),
                );
            }
        }
    }

    let toolchain_version = m.get_one::<String>("toolchain-version").map(Into::into);
    if let Some(toolchain_version) = &toolchain_version {
        if !unstable_flags.contains(&UnstableFlags::ToolchainVersion) {
            return Err(
                    format!("`--toolchain-version={toolchain_version}` is unstable and requires `-Z toolchain-version` (epage/cargo-script-mvs#36).")
                        .into(),
                );
        }
    }

    let expr = *m.get_one::<bool>("expr").expect("defaulted");
    if expr && !unstable_flags.contains(&UnstableFlags::Expr) {
        return Err(
            "`--expr` is unstable and requires `-Z expr` (epage/cargo-script-mvs#72).".into(),
        );
    }

    Ok(Args {
        script,
        script_args,
        features: m
            .get_many::<String>("features")
            .unwrap_or_default()
            .map(|s| s.to_owned())
            .collect(),

        expr,

        pkg_path: m.get_one::<PathBuf>("pkg_path").map(Into::into),
        cargo_output: *m.get_one::<bool>("cargo-output").expect("defaulted"),
        clear_cache: *m.get_one::<bool>("clear-cache").expect("defaulted"),
        debug: !m.get_flag("release"),
        force: *m.get_one::<bool>("force").expect("defaulted"),
        build_kind,
        template: m.get_one::<String>("template").map(Into::into),
        list_templates: *m.get_one::<bool>("list-templates").expect("defaulted"),
        toolchain_version,
        #[cfg(windows)]
        install_file_association: *m
            .get_one::<bool>("install-file-association")
            .expect("defaulted"),
        #[cfg(windows)]
        uninstall_file_association: *m
            .get_one::<bool>("uninstall-file-association")
            .expect("defaulted"),
        unstable_flags,
    })
}

fn main() {
    env_logger::init();

    let stderr = &mut std::io::stderr();

    match try_main() {
        Ok(0) => (),
        Ok(code) => {
            std::process::exit(code);
        }
        Err(ref err) => {
            writeln!(stderr, "error: {err}").unwrap();
            std::process::exit(1);
        }
    }
}

fn try_main() -> MainResult<i32> {
    let args = parse_args()?;
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

    if args.list_templates {
        templates::list()?;
        return Ok(0);
    }

    // Take the arguments and work out what our input is going to be.  Primarily, this gives us the content, a user-friendly name, and a cache-friendly ID.
    // These three are just storage for the borrows we'll actually use.
    let script_name: String;
    let script_path: PathBuf;

    let input = match (args.script.clone().unwrap(), args.expr) {
        (script, false) => {
            let (path, mut file) = find_script(&script).ok_or(format!(
                "could not find script: {}",
                script.to_string_lossy()
            ))?;

            script_name = path
                .file_stem()
                .map(|os| os.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());

            let mut body = String::new();
            file.read_to_string(&mut body)?;

            script_path = std::env::current_dir()?.join(path);

            Input::File(script_name, script_path, body)
        }
        (expr, true) => {
            let expr = expr
                .to_str()
                .ok_or_else(|| format!("expr must be UTF-8, got {}", expr.to_string_lossy()))?
                .to_owned();
            Input::Expr(expr, args.template.clone())
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
        Defer::<_, MainError>::new(move || {
            if !cc {
                gc_cache(MAX_CACHE_AGE)?;
            }
            Ok(())
        })
    };

    let exit_code = {
        let cmd_name = action.build_kind.exec_command();
        log::trace!("running `cargo {}`", cmd_name);

        let run_quietly = !action.cargo_output;
        let mut cmd = action.cargo(cmd_name, &args.script_args, run_quietly)?;

        cmd.status().map(|st| st.code().unwrap_or(1))?
    };

    Ok(exit_code)
}

/// How old can stuff in the cache be before we automatically clear it out?
pub const MAX_CACHE_AGE: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// Empty the cache
fn clean_cache() -> MainResult<()> {
    log::info!("cleaning cache");

    let cache_dir = platform::binary_cache_path()?;
    if ALLOW_AUTO_REMOVE && cache_dir.exists() {
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
fn gc_cache(max_age: std::time::Duration) -> MainResult<()> {
    log::info!("cleaning cache with max_age: {:?}", max_age);

    let cutoff = std::time::SystemTime::now() - max_age;
    log::trace!("cutoff:     {:>20?} ms", cutoff);

    let cache_dir = platform::generated_projects_cache_path()?;
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
                if ALLOW_AUTO_REMOVE {
                    if let Err(err) = fs::remove_dir_all(&path) {
                        log::error!("failed to remove {:?} from cache: {}", path, err);
                    }
                } else {
                    log::debug!("(suppressed remove)");
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
fn gen_pkg_and_compile(input: &Input, action: &InputAction) -> MainResult<()> {
    let pkg_path = &action.pkg_path;
    let meta = &action.metadata;
    let old_meta = action.old_metadata.as_ref();

    let mani_str = &action.manifest;
    let script_str = &action.script;

    log::trace!("creating pkg dir...");
    fs::create_dir_all(pkg_path)?;
    let cleanup_dir: Defer<_, MainError> = Defer::new(|| {
        // DO NOT try deleting ANYTHING if we're not cleaning up inside our own cache.  We *DO NOT* want to risk killing user files.
        if action.using_cache {
            log::debug!("cleaning up cache directory {}", pkg_path.display());
            if ALLOW_AUTO_REMOVE {
                fs::remove_dir_all(pkg_path)?;
            } else {
                log::debug!("(suppressed remove)");
            }
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

    fn cargo(&self, cmd: &str, script_args: &[OsString], run_quietly: bool) -> MainResult<Command> {
        cargo(
            cmd,
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

    /// Template used.
    template: Option<String>,

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
fn decide_action_for(input: &Input, args: &Args) -> MainResult<InputAction> {
    let input_id = input.compute_id();
    log::trace!("id: {:?}", input_id);

    let (pkg_path, using_cache) = args
        .pkg_path
        .as_ref()
        .map(|p| (p.into(), false))
        .unwrap_or_else(|| {
            // This can't fail.  Seriously, we're *fucked* if we can't work this out.
            let cache_path = platform::generated_projects_cache_path().unwrap();
            (cache_path.join(&input_id), true)
        });
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
        let (path, template) = match input {
            Input::File(_, path, _) => (Some(path.to_string_lossy().into_owned()), None),
            Input::Expr(_, template) => (None, template.clone()),
        };
        let features = if args.features.is_empty() {
            None
        } else {
            Some(args.features.join(" "))
        };
        PackageMetadata {
            path,
            template,
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

    macro_rules! bail {
        ($($name:ident: $value:expr),*) => {
            return Ok(InputAction {
                $($name: $value,)*
                ..action
            })
        }
    }

    // If we're not doing a regular build, stop.
    match action.build_kind {
        BuildKind::Normal => (),
        BuildKind::Test | BuildKind::Bench => {
            log::debug!("not recompiling because: user asked for test/bench");
            bail!(force_compile: false)
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
fn get_pkg_metadata<P>(pkg_path: P) -> MainResult<PackageMetadata>
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
    let meta: PackageMetadata = serde_json::from_str(&meta_str).map_err(|err| err.to_string())?;

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
fn write_pkg_metadata<P>(pkg_path: P, meta: &PackageMetadata) -> MainResult<()>
where
    P: AsRef<Path>,
{
    let meta_path = get_pkg_metadata_path(&pkg_path);
    log::trace!("meta_path: {:?}", meta_path);
    let mut temp_file = tempfile::NamedTempFile::new_in(&pkg_path)?;
    serde_json::to_writer(BufWriter::new(&temp_file), meta).map_err(|err| err.to_string())?;
    temp_file.flush()?;
    temp_file
        .persist(&meta_path)
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Attempts to locate the script specified by the given path.  If the path as-given doesn't yield anything, it will try adding file extensions.
fn find_script<P>(path: P) -> Option<(PathBuf, fs::File)>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    // Try the path directly.
    if let Ok(file) = fs::File::open(path) {
        return Some((path.into(), file));
    }

    // If it had an extension, don't bother trying any others.
    if path.extension().is_some() {
        return None;
    }

    // Ok, now try other extensions.
    for ext in ["ers", "rs"] {
        let path = path.with_extension(ext);
        if let Ok(file) = fs::File::open(&path) {
            return Some((path, file));
        }
    }

    // Welp. ¯\_(ツ)_/¯
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
    Expr(String, Option<String>),
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
            Self::Expr(content, template) => {
                let mut hasher = Sha1::new();

                hasher.update("template:");
                hasher.update(template.as_deref().unwrap_or(""));
                hasher.update(";");

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
fn overwrite_file<P>(path: P, content: &str, hash: Option<&str>) -> MainResult<FileOverwrite>
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
        .ok_or("The given path should be a file")?;
    let mut temp_file = tempfile::NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    temp_file.persist(path).map_err(|e| e.to_string())?;
    Ok(FileOverwrite::Changed { new_hash })
}

/// Constructs a Cargo command that runs on the script package.
fn cargo(
    cmd_name: &str,
    manifest: &str,
    toolchain_version: Option<&str>,
    meta: &PackageMetadata,
    script_args: &[OsString],
    run_quietly: bool,
) -> MainResult<Command> {
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

    cmd.arg(cmd_name);

    if cmd_name == "run" && run_quietly {
        cmd.arg("-q");
    }

    cmd.arg("--manifest-path").arg(manifest);

    if platform::force_cargo_color() {
        cmd.arg("--color").arg("always");
    }

    let cargo_target_dir = format!("{}", platform::binary_cache_path()?.display(),);
    cmd.arg("--target-dir");
    cmd.arg(cargo_target_dir);

    // Block `--release` on `bench`.
    if !meta.debug && cmd_name != "bench" {
        cmd.arg("--release");
    }

    if let Some(ref features) = meta.features {
        cmd.arg("--features").arg(features);
    }

    if cmd_name == "run" && !script_args.is_empty() {
        cmd.arg("--");
        cmd.args(script_args.iter());
    }

    Ok(cmd)
}

#[test]
fn test_package_name() {
    let input = Input::File("Script".into(), Path::new("path").into(), "script".into());
    assert_eq!("script", input.package_name());
    let input = Input::File("1Script".into(), Path::new("path").into(), "script".into());
    assert_eq!("_1script", input.package_name());
}
