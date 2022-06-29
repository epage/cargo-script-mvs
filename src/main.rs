#![forbid(unsafe_code)]

/**
If this is set to `false`, then code that automatically deletes stuff *won't*.
*/
const ALLOW_AUTO_REMOVE: bool = true;

mod consts;
mod error;
mod manifest;
mod platform;
mod templates;
mod util;

#[cfg(windows)]
mod file_assoc;

#[cfg(not(windows))]
mod file_assoc {}

use log::{debug, error, info};
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
    script: Option<String>,
    script_args: Vec<String>,
    features: Option<String>,

    expr: bool,
    loop_: bool,
    count: bool,

    pkg_path: Option<String>,
    gen_pkg_only: bool,
    cargo_output: bool,
    clear_cache: bool,
    debug: bool,
    force: bool,
    unstable_features: Vec<String>,
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

fn parse_args() -> Args {
    use clap::{Arg, ArgGroup, Command};
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let about = r#"Compiles and runs a Rust script."#;

    let app = Command::new(consts::PROGRAM_NAME)
        .version(version)
        .about(about)
        .trailing_var_arg(true)
            .arg(Arg::new("script")
                .index(1)
                .help("Script file or expression to execute.")
                .required_unless_present_any(if cfg!(windows) {
                    vec!["clear-cache", "list-templates", "install-file-association", "uninstall-file-association"]
                } else {
                    vec!["clear-cache", "list-templates"]
                })
                .conflicts_with_all(if cfg!(windows) {
                    &["list-templates", "install-file-association", "uninstall-file-association"]
                } else {
                    &["list-templates"]
                })
                .multiple_values(true)
            )
            .arg(Arg::new("expr")
                .help("Execute <script> as a literal expression and display the result.")
                .long("expr")
                .short('e')
                .takes_value(false)
                .requires("script")
            )
            .arg(Arg::new("loop")
                .help("Execute <script> as a literal closure once for each line from stdin.")
                .long("loop")
                .short('l')
                .takes_value(false)
                .requires("script")
            )
            .group(ArgGroup::new("expr_or_loop")
                .args(&["expr", "loop"])
            )
            /*
            Options that impact the script being executed.
            */
            .arg(Arg::new("cargo-output")
                .help("Show output from cargo when building.")
                .short('o')
                .long("cargo-output")
                .requires("script")
            )
            .arg(Arg::new("count")
                .help("Invoke the loop closure with two arguments: line, and line number.")
                .long("count")
                .requires("loop")
            )
            .arg(Arg::new("release")
                .help("Build a release executable, an optimised one.")
                .short('r')
                .long("release")
                .conflicts_with_all(&["bench"])
            )
            .arg(Arg::new("features")
                 .help("Cargo features to pass when building and running.")
                 .long("features")
                 .takes_value(true)
            )
            .arg(Arg::new("unstable_features")
                .help("Add a #![feature] declaration to the crate.")
                .long("unstable-feature")
                .short('u')
                .takes_value(true)
                .multiple_occurrences(true)
                .requires("expr_or_loop")
            )

            /*
            Options that change how rust-script itself behaves, and don't alter what the script will do.
            */
            .arg(Arg::new("clear-cache")
                .help("Clears out the script cache.")
                .long("clear-cache")
            )
            .arg(Arg::new("force")
                .help("Force the script to be rebuilt.")
                .long("force")
                .requires("script")
            )
            .arg(Arg::new("gen_pkg_only")
                .help("Generate the Cargo package, but don't compile or run it.")
                .long("gen-pkg-only")
                .requires("script")
                .conflicts_with_all(&["release", "force", "test", "bench"])
            )
            .arg(Arg::new("pkg_path")
                .help("Specify where to place the generated Cargo package.")
                .long("pkg-path")
                .takes_value(true)
                .requires("script")
                .conflicts_with_all(&["clear-cache", "force"])
            )
            .arg(Arg::new("test")
                .help("Compile and run tests.")
                .long("test")
                .conflicts_with_all(&["bench", "force"])
            )
            .arg(Arg::new("bench")
                .help("Compile and run benchmarks. Requires a nightly toolchain.")
                .long("bench")
                .conflicts_with_all(&["test", "force"])
            )
            .arg(Arg::new("template")
                .help("Specify a template to use for expression scripts.")
                .long("template")
                .short('t')
                .takes_value(true)
                .requires("expr")
            )
            .arg(Arg::new("toolchain-version")
                .help("Build the script using the given toolchain version.")
                .long("toolchain-version")
                // "channel"
                .short('c')
                .takes_value(true)
                // FIXME: remove if benchmarking is stabilized
                .conflicts_with("bench")
            )
        .arg(Arg::new("list-templates")
            .help("List the available templates.")
            .long("list-templates")
            .takes_value(false)
        );

    #[cfg(windows)]
    let app = app
        .arg(
            Arg::new("install-file-association")
                .help("Install a file association so that rust-script executes .ers files.")
                .long("install-file-association"),
        )
        .arg(
            Arg::new("uninstall-file-association")
                .help("Uninstall the file association that makes rust-script execute .ers files.")
                .long("uninstall-file-association"),
        )
        .group(
            ArgGroup::new("file-association")
                .args(&["install-file-association", "uninstall-file-association"]),
        );

    let m = app.get_matches();

    fn owned_vec_string<'a, I>(v: Option<I>) -> Vec<String>
    where
        I: ::std::iter::Iterator<Item = &'a str>,
    {
        v.map(|itr| itr.map(Into::into).collect())
            .unwrap_or_default()
    }

    let script_and_args: Option<Vec<&str>> = m.values_of("script").map(|o| o.collect());
    let script;
    let script_args: Vec<String>;
    if let Some(script_and_args) = script_and_args {
        script = script_and_args.get(0).map(|s| s.to_string());
        script_args = if script_and_args.len() > 1 {
            script_and_args[1..].iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };
    } else {
        script = None;
        script_args = Vec::new();
    }

    Args {
        script,
        script_args,
        features: m.value_of("features").map(Into::into),

        expr: m.is_present("expr"),
        loop_: m.is_present("loop"),
        count: m.is_present("count"),

        pkg_path: m.value_of("pkg_path").map(Into::into),
        gen_pkg_only: m.is_present("gen_pkg_only"),
        cargo_output: m.is_present("cargo-output"),
        clear_cache: m.is_present("clear-cache"),
        debug: !m.is_present("release"),
        force: m.is_present("force"),
        unstable_features: owned_vec_string(m.values_of("unstable_features")),
        build_kind: BuildKind::from_flags(m.is_present("test"), m.is_present("bench")),
        template: m.value_of("template").map(Into::into),
        list_templates: m.is_present("list-templates"),
        toolchain_version: m.value_of("toolchain-version").map(Into::into),
        #[cfg(windows)]
        install_file_association: m.is_present("install-file-association"),
        #[cfg(windows)]
        uninstall_file_association: m.is_present("uninstall-file-association"),
    }
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
            writeln!(stderr, "error: {}", err).unwrap();
            std::process::exit(1);
        }
    }
}

fn try_main() -> MainResult<i32> {
    let args = parse_args();
    info!("Arguments: {:?}", args);

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
        clean_cache(0)?;
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
    let content: String;

    let input = match (args.script.clone().unwrap(), args.expr, args.loop_) {
        (script, false, false) => {
            let (path, mut file) =
                find_script(&script).ok_or(format!("could not find script: {}", script))?;

            script_name = path
                .file_stem()
                .map(|os| os.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());

            let mut body = String::new();
            file.read_to_string(&mut body)?;

            let mtime = platform::file_last_modified(&file);

            script_path = std::env::current_dir()?.join(path);
            content = body;

            Input::File(&script_name, &script_path, &content, mtime)
        }
        (expr, true, false) => {
            content = expr;
            Input::Expr(&content, args.template.as_deref())
        }
        (loop_, false, true) => {
            content = loop_;
            Input::Loop(&content, args.count)
        }
        (_, _, _) => {
            panic!("Internal error: Invalid args");
        }
    };
    info!("input: {:?}", input);

    // Setup environment variables early so it's available at compilation time of scripts,
    // to allow e.g. include!(concat!(env!("RUST_SCRIPT_BASE_PATH"), "/script-module.rs"));
    std::env::set_var(
        "RUST_SCRIPT_PATH",
        input.path().unwrap_or_else(|| Path::new("")),
    );
    std::env::set_var("RUST_SCRIPT_SAFE_NAME", input.safe_name());
    std::env::set_var("RUST_SCRIPT_PKG_NAME", input.package_name());
    std::env::set_var("RUST_SCRIPT_BASE_PATH", input.base_path());

    // Generate the prelude items, if we need any. Ensure consistent and *valid* sorting.
    let prelude_items = {
        let unstable_features = args
            .unstable_features
            .iter()
            .map(|uf| format!("#![feature({})]", uf));

        let mut items: Vec<_> = unstable_features.collect();
        items.sort();
        items
    };
    info!("prelude_items: {:?}", prelude_items);

    
    let action = decide_action_for(&input, prelude_items, &args)?;
    info!("action: {:?}", action);

    gen_pkg_and_compile(&input, &action)?;

    // Once we're done, clean out old packages from the cache.
    // There's no point if we've already done a full clear, though.
    let _defer_clear = {
        // To get around partially moved args problems.
        let cc = args.clear_cache;
        Defer::<_, MainError>::new(move || {
            if !cc {
                clean_cache(consts::MAX_CACHE_AGE_MS)?;
            }
            Ok(())
        })
    };

    let exit_code = if action.execute {
        let cmd_name = action.build_kind.exec_command();
        info!("running `cargo {}`", cmd_name);
        let run_quietly = !action.cargo_output;
        let mut cmd = action.cargo(cmd_name, &args.script_args, run_quietly)?;

        cmd.status().map(|st| st.code().unwrap_or(1))?
    } else {
        0
    };

    Ok(exit_code)
}

/**
Clean up the cache folder.

Looks for all folders whose metadata says they were created at least `max_age` in the past and kills them dead.
*/
fn clean_cache(max_age: u128) -> MainResult<()> {
    info!("cleaning cache with max_age: {:?}", max_age);

    if max_age == 0 {
        info!("max_age is 0, clearing binary cache...");
        let cache_dir = platform::binary_cache_path()?;
        if ALLOW_AUTO_REMOVE {
            if let Err(err) = fs::remove_dir_all(&cache_dir) {
                error!("failed to remove binary cache {:?}: {}", cache_dir, err);
            }
        }
    }

    let cutoff = platform::current_time() - max_age;
    info!("cutoff:     {:>20?} ms", cutoff);

    let cache_dir = platform::generated_projects_cache_path()?;
    for child in fs::read_dir(cache_dir)? {
        let child = child?;
        let path = child.path();
        if path.is_file() {
            continue;
        }

        info!("checking: {:?}", path);

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
                let meta_file = match fs::File::open(&meta_path) {
                    Ok(file) => file,
                    Err(..) => {
                        info!("couldn't open metadata for {:?}", path);
                        return true;
                    }
                };
                platform::file_last_modified(&meta_file)
            };
            info!("meta_mtime: {:>20?} ms", meta_mtime);

            meta_mtime <= cutoff
        };

        if remove_dir() {
            info!("removing {:?}", path);
            if ALLOW_AUTO_REMOVE {
                if let Err(err) = fs::remove_dir_all(&path) {
                    error!("failed to remove {:?} from cache: {}", path, err);
                }
            } else {
                info!("(suppressed remove)");
            }
        }
    }
    info!("done cleaning cache.");
    Ok(())
}

/**
Generate and compile a package from the input.

Why take `PackageMetadata`?  To ensure that any information we need to depend on for compilation *first* passes through `decide_action_for` *and* is less likely to not be serialised with the rest of the metadata.
*/
fn gen_pkg_and_compile(input: &Input, action: &InputAction) -> MainResult<()> {
    let pkg_path = &action.pkg_path;
    let meta = &action.metadata;
    let old_meta = action.old_metadata.as_ref();

    let mani_str = &action.manifest;
    let script_str = &action.script;

    info!("creating pkg dir...");
    fs::create_dir_all(pkg_path)?;
    let cleanup_dir: Defer<_, MainError> = Defer::new(|| {
        // DO NOT try deleting ANYTHING if we're not cleaning up inside our own cache.  We *DO NOT* want to risk killing user files.
        if action.using_cache {
            info!("cleaning up cache directory {:?}", pkg_path);
            if ALLOW_AUTO_REMOVE {
                fs::remove_dir_all(pkg_path)?;
            } else {
                info!("(suppressed remove)");
            }
        }
        Ok(())
    });

    let mut meta = meta.clone();

    info!("generating Cargo package...");
    let mani_path = action.manifest_path();
    let mani_hash = old_meta.map(|m| &*m.manifest_hash);
    match overwrite_file(&mani_path, mani_str, mani_hash)? {
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
            debug!("told to force compile, ignoring script hash");
            None
        } else {
            old_meta.map(|m| &*m.script_hash)
        };
        match overwrite_file(&script_path, script_str, script_hash)? {
            FileOverwrite::Same => (),
            FileOverwrite::Changed { new_hash } => {
                meta.script_hash = new_hash;
            }
        }
    }

    let meta = meta;

    // Write out metadata *now*.  Remember that we check the timestamp in the metadata, *not* on the executable.
    if action.emit_metadata {
        info!("emitting metadata...");
        write_pkg_metadata(pkg_path, &meta)?;
    }

    info!("disarming pkg dir cleanup...");
    cleanup_dir.disarm();

    Ok(())
}

/**
This represents what to do with the input provided by the user.
*/
#[derive(Debug)]
struct InputAction {
    /// Always show cargo output?
    cargo_output: bool,

    /**
    Force Cargo to do a recompile, even if it thinks it doesn't have to.

    `compile` must be `true` for this to have any effect.
    */
    force_compile: bool,

    /// Emit a metadata file?
    emit_metadata: bool,

    /// Execute the compiled binary?
    execute: bool,

    /// Directory where the package should live.
    pkg_path: PathBuf,

    /**
    Is the package directory in the cache?

    Currently, this can be inferred from `emit_metadata`, but there's no *intrinsic* reason they should be tied together.
    */
    using_cache: bool,

    /**
    Which toolchain the script should be built with.

    `None` indicates that the script should be built with a stable toolchain.
    */
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

    fn cargo(&self, cmd: &str, script_args: &[String], run_quietly: bool) -> MainResult<Command> {
        cargo(
            cmd,
            &*self.manifest_path().to_string_lossy(),
            self.toolchain_version.as_deref(),
            &self.metadata,
            script_args,
            run_quietly,
        )
    }
}

/**
The metadata here serves two purposes:

1. It records everything necessary for compilation and execution of a package.
2. It records everything that must be exactly the same in order for a cached executable to still be valid, in addition to the content hash.
*/
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PackageMetadata {
    /// Path to the script file.
    path: Option<String>,

    /// Last-modified timestamp for script file.
    modified: Option<u128>,

    /// Template used.
    template: Option<String>,

    /// Was the script compiled in debug mode?
    debug: bool,

    /// Sorted list of dependencies.
    deps: Vec<(String, String)>,

    /// Sorted list of injected prelude items.
    prelude: Vec<String>,

    /// Cargo features
    features: Option<String>,

    /// Hash of the generated `Cargo.toml` file.
    manifest_hash: String,

    /// Hash of the generated source file.
    script_hash: String,
}

/**
For the given input, this constructs the package metadata and checks the cache to see what should be done.
*/
fn decide_action_for(
    input: &Input,
    prelude: Vec<String>,
    args: &Args,
) -> MainResult<InputAction> {
    /// Placeholder DS to be mopped up with changes to manifest.rs syntax #37
    use std::collections::HashMap;
    let tmp_dep: HashMap<String, String> = HashMap::new();
    let deps: Vec<(String, String)> = tmp_dep.into_iter().collect();

    let input_id = {
        let deps_iter = deps.iter().map(|&(ref n, ref v)| (n as &str, v as &str));
        // Again, also fucked if we can't work this out.
        input.compute_id(deps_iter).unwrap()
    };
    info!("id: {:?}", input_id);

    let (pkg_path, using_cache) = args
        .pkg_path
        .as_ref()
        .map(|p| (p.into(), false))
        .unwrap_or_else(|| {
            // This can't fail.  Seriously, we're *fucked* if we can't work this out.
            let cache_path = platform::generated_projects_cache_path().unwrap();
            (cache_path.join(&input_id), true)
        });
    info!("pkg_path: {:?}", pkg_path);
    info!("using_cache: {:?}", using_cache);

    let (mani_str, script_str) = manifest::split_input(input, &deps, &prelude, &input_id)?;

    // Forcibly override some flags based on build kind.
    let (debug, force) = match args.build_kind {
        BuildKind::Normal => (args.debug, args.force),
        BuildKind::Test => (true, false),
        BuildKind::Bench => (false, false),
    };

    let input_meta = {
        let (path, mtime, template) = match *input {
            Input::File(_, path, _, mtime) => {
                (Some(path.to_string_lossy().into_owned()), Some(mtime), None)
            }
            Input::Expr(_, template) => (None, None, template),
            Input::Loop(..) => (None, None, None),
        };
        PackageMetadata {
            path,
            modified: mtime,
            template: template.map(Into::into),
            debug,
            deps,
            prelude,
            features: args.features.clone(),
            manifest_hash: hash_str(&mani_str),
            script_hash: hash_str(&script_str),
        }
    };
    info!("input_meta: {:?}", input_meta);

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
        execute: true,
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

    // If we were told to only generate the package, we need to stop *now*
    if args.gen_pkg_only {
        bail!(execute: false)
    }

    // If we're not doing a regular build, stop.
    match action.build_kind {
        BuildKind::Normal => (),
        BuildKind::Test | BuildKind::Bench => {
            info!("not recompiling because: user asked for test/bench");
            bail!(force_compile: false)
        }
    }

    action.old_metadata = match get_pkg_metadata(&action.pkg_path) {
        Ok(meta) => Some(meta),
        Err(err) => {
            info!(
                "recompiling since failed to load metadata: {}",
                err.to_string()
            );
            None
        }
    };

    Ok(action)
}

/**
Load the package metadata, given the path to the package's cache folder.
*/
fn get_pkg_metadata<P>(pkg_path: P) -> MainResult<PackageMetadata>
where
    P: AsRef<Path>,
{
    let meta_path = get_pkg_metadata_path(pkg_path);
    debug!("meta_path: {:?}", meta_path);
    let mut meta_file = fs::File::open(&meta_path)?;

    let meta_str = {
        let mut s = String::new();
        meta_file.read_to_string(&mut s).unwrap();
        s
    };
    let meta: PackageMetadata = serde_json::from_str(&meta_str).map_err(|err| err.to_string())?;

    Ok(meta)
}

/**
Work out the path to a package's metadata file.
*/
fn get_pkg_metadata_path<P>(pkg_path: P) -> PathBuf
where
    P: AsRef<Path>,
{
    pkg_path.as_ref().join(consts::METADATA_FILE)
}

/**
Save the package metadata, given the path to the package's cache folder.
*/
fn write_pkg_metadata<P>(pkg_path: P, meta: &PackageMetadata) -> MainResult<()>
where
    P: AsRef<Path>,
{
    let meta_path = get_pkg_metadata_path(&pkg_path);
    debug!("meta_path: {:?}", meta_path);
    let mut temp_file = tempfile::NamedTempFile::new_in(&pkg_path)?;
    serde_json::to_writer(BufWriter::new(&temp_file), meta).map_err(|err| err.to_string())?;
    temp_file.flush()?;
    temp_file
        .persist(&meta_path)
        .map_err(|err| err.to_string())?;
    Ok(())
}

/**
Attempts to locate the script specified by the given path.  If the path as-given doesn't yield anything, it will try adding file extensions.
*/
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
    for &ext in consts::SEARCH_EXTS {
        let path = path.with_extension(ext);
        if let Ok(file) = fs::File::open(&path) {
            return Some((path, file));
        }
    }

    // Welp. ¯\_(ツ)_/¯
    None
}

/**
Represents an input source for a script.
*/
#[derive(Clone, Debug)]
pub enum Input<'a> {
    /**
    The input is a script file.

    The tuple members are: the name, absolute path, script contents, last modified time.
    */
    File(&'a str, &'a Path, &'a str, u128),

    /**
    The input is an expression.

    The tuple member is: the script contents, and the template (if any).
    */
    Expr(&'a str, Option<&'a str>),

    /**
    The input is a loop expression.

    The tuple member is: the script contents, whether the `--count` flag was given.
    */
    Loop(&'a str, bool),
}

impl<'a> Input<'a> {
    /**
    Return the path to the script, if it has one.
    */
    pub const fn path(&self) -> Option<&Path> {
        use crate::Input::*;

        match *self {
            File(_, path, _, _) => Some(path),
            Expr(..) => None,
            Loop(..) => None,
        }
    }

    /**
    Return the "safe name" for the input.  This should be filename-safe.

    Currently, nothing is done to ensure this, other than hoping *really hard* that we don't get fed some excessively bizzare input filename.
    */
    pub const fn safe_name(&self) -> &str {
        use crate::Input::*;

        match *self {
            File(name, _, _, _) => name,
            Expr(..) => "expr",
            Loop(..) => "loop",
        }
    }

    /**
    Return the package name for the input.  This should be a valid Rust identifier.
    */
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

    /**
    Base directory for resolving relative paths.
    */
    pub fn base_path(&self) -> PathBuf {
        match *self {
            Input::File(_, path, _, _) => path
                .parent()
                .expect("couldn't get parent directory for file input base path")
                .into(),
            Input::Expr(..) | Input::Loop(..) => {
                std::env::current_dir().expect("couldn't get current directory for input base path")
            }
        }
    }

    // Compute the package ID for the input.
    // This is used as the name of the cache folder into which the Cargo package
    // will be generated.
    pub fn compute_id<'dep, DepIt>(&self, deps: DepIt) -> MainResult<OsString>
    where
        DepIt: IntoIterator<Item = (&'dep str, &'dep str)>,
    {
        use crate::Input::*;

        let hash_deps = || {
            let mut hasher = Sha1::new();
            for dep in deps {
                hasher.update(b"dep=");
                hasher.update(dep.0);
                hasher.update(b"=");
                hasher.update(dep.1);
                hasher.update(b";");
            }
            hasher
        };

        match *self {
            File(_, path, _, _) => {
                let mut hasher = Sha1::new();

                // Hash the path to the script.
                hasher.update(&*path.to_string_lossy());
                let mut digest = format!("{:x}", hasher.finalize());
                digest.truncate(consts::ID_DIGEST_LEN_MAX);

                let mut id = OsString::new();
                id.push(&*digest);
                Ok(id)
            }
            Expr(content, template) => {
                let mut hasher = hash_deps();

                hasher.update("template:");
                hasher.update(template.unwrap_or(""));
                hasher.update(";");

                hasher.update(content);
                let mut digest = format!("{:x}", hasher.finalize());
                digest.truncate(consts::ID_DIGEST_LEN_MAX);

                let mut id = OsString::new();
                id.push(&*digest);
                Ok(id)
            }
            Loop(content, count) => {
                let mut hasher = hash_deps();

                // Make sure to include the [non-]presence of the `--count` flag in the flag, since it changes the actual generated script output.
                hasher.update("count:");
                hasher.update(if count { "true;" } else { "false;" });

                hasher.update(content);
                let mut digest = format!("{:x}", hasher.finalize());
                digest.truncate(consts::ID_DIGEST_LEN_MAX);

                let mut id = OsString::new();
                id.push(&*digest);
                Ok(id)
            }
        }
    }
}

/**
Shorthand for hashing a string.
*/
fn hash_str(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s);
    format!("{:x}", hasher.finalize())
}

enum FileOverwrite {
    Same,
    Changed { new_hash: String },
}

/**
Overwrite a file if and only if the contents have changed.
*/
fn overwrite_file<P>(path: P, content: &str, hash: Option<&str>) -> MainResult<FileOverwrite>
where
    P: AsRef<Path>,
{
    debug!("overwrite_file({:?}, _, {:?})", path.as_ref(), hash);
    let new_hash = hash_str(content);
    if Some(&*new_hash) == hash {
        debug!(".. hashes match");
        return Ok(FileOverwrite::Same);
    }

    debug!(".. hashes differ; new_hash: {:?}", new_hash);
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

/**
Constructs a Cargo command that runs on the script package.
*/
fn cargo(
    cmd_name: &str,
    manifest: &str,
    toolchain_version: Option<&str>,
    meta: &PackageMetadata,
    script_args: &[String],
    run_quietly: bool,
) -> MainResult<Command> {
    let mut cmd = Command::new("cargo");

    // Always specify a toolchain to avoid being affected by rust-version(.toml) files:
    cmd.arg(format!("+{}", toolchain_version.unwrap_or("stable")));

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
    let input = Input::File("Script", Path::new("path"), "script", 0);
    assert_eq!("script", input.package_name());
    let input = Input::File("1Script", Path::new("path"), "script", 0);
    assert_eq!("_1script", input.package_name());
}
