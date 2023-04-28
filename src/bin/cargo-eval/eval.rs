use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context as _;
use cargo::util::errors::CliError;

use cargo_eval::config::UnstableFlags;
use cargo_eval::CargoResult;
use cargo_eval::CliResult;

pub fn cli() -> clap::Command {
    clap::command!()
        .name("cargo-eval")
        .args([
            clap::Arg::new("script")
                .num_args(1..)
                .value_names(["PATH_RS", "ARG"])
                .trailing_var_arg(true)
                .value_parser(clap::value_parser!(OsString))
                .help("Script file or expression to execute"),
            clap::Arg::new("release")
                .short('r')
                .long("release")
                .action(clap::ArgAction::SetTrue)
                .help("Build a release executable, an optimised one")
                .conflicts_with_all(["bench"]),
            clap::Arg::new("target-dir")
                .long("target-dir")
                .value_name("DIRECTORY")
                .help("Directory for all generated artifacts"),
            // Options that impact the script being executed.
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::Count)
                .help("Use verbose output"),
        ])
        // Options that change how rust-script itself behaves, and don't alter what the script will do.
        .args([
            clap::Arg::new("clean")
                .long("clean")
                .help("Remove the script target directory (unstable)")
                .help_heading("Polyfill")
                .action(clap::ArgAction::SetTrue)
                .requires("script")
                .group("action"),
            clap::Arg::new("test")
                .long("test")
                .action(clap::ArgAction::SetTrue)
                .help("Compile and run tests (unstable)")
                .help_heading("Polyfill")
                .requires("script")
                .group("action"),
            clap::Arg::new("bench")
                .long("bench")
                .action(clap::ArgAction::SetTrue)
                .help("Compile and run benchmarks (unstable)")
                .help_heading("Polyfill")
                .requires("script")
                .group("action"),
            clap::Arg::new("unstable_flags")
                .short('Z')
                .value_name("FLAG")
                .value_parser(clap::value_parser!(UnstableFlags))
                .action(clap::ArgAction::Append)
                .help("Unstable (nightly-only) flags"),
        ])
}

pub fn exec(matches: &clap::ArgMatches, config: &mut cargo::util::Config) -> CliResult {
    let unstable_flags = matches
        .get_many::<UnstableFlags>("unstable_flags")
        .unwrap_or_default()
        .copied()
        .collect::<Vec<_>>();

    let action = if matches.get_flag("clean") {
        if !unstable_flags.contains(&UnstableFlags::Polyfill) {
            return Err(
                anyhow::format_err!("`--clean` is unstable and requires `-Zpolyfill`").into(),
            );
        }
        Action::Clean
    } else if matches.get_flag("test") {
        if !unstable_flags.contains(&UnstableFlags::Polyfill) {
            return Err(
                anyhow::format_err!("`--test` is unstable and requires `-Zpolyfill`").into(),
            );
        }
        Action::Test
    } else if matches.get_flag("bench") {
        if !unstable_flags.contains(&UnstableFlags::Polyfill) {
            return Err(
                anyhow::format_err!("`--bench` is unstable and requires `-Zpolyfill`").into(),
            );
        }
        Action::Bench
    } else {
        Action::Run
    };

    let mut script_and_args = matches
        .get_many::<OsString>("script")
        .unwrap_or_default()
        .cloned();
    let script = script_and_args.next();
    let script = if let Some(script) = script {
        script
    } else {
        use is_terminal::IsTerminal;
        if std::io::stdin().is_terminal() {
            return Err(anyhow::format_err!("<PATH_RS> is required").into());
        } else {
            "-".into()
        }
    };
    let script_args: Vec<OsString> = script_and_args.collect();

    let release = matches.get_flag("release");

    let verbose = matches.get_count("verbose");
    let (verbose, quiet) = if matches!(&action, Action::Run) {
        verbose
            .checked_sub(1)
            .map(|v| (v, false))
            .unwrap_or((0, true))
    } else {
        (verbose, false)
    };
    let color = None;
    let frozen = false;
    let locked = false;
    let offline = false;
    // HACK: We should only pass in `--target-dir` to config **but** we need to make sure that
    // `default_target_dir` is used instead of one derived from the `Workspace`s location.  If/when
    // upstreamed into cargo, instead `Workspace` would recognize that its using an embedded
    // manifest and would instead choose `default_target_dir` for us.
    let target_dir = matches
        .get_one::<PathBuf>("target-dir")
        .cloned()
        .or_else(|| std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from))
        .map(Ok)
        .unwrap_or_else(cargo_eval::config::default_target_dir)?;
    let cli_config = [];
    config.configure(
        verbose as u32,
        quiet,
        color,
        frozen,
        locked,
        offline,
        &Some(target_dir),
        &[],
        &cli_config,
    )?;

    match action {
        Action::Run => {
            if std::env::var_os("RUST_BACKTRACE").is_none() {
                std::env::set_var("RUST_BACKTRACE", "1");
            }
            let manifest_path = if script == "-" {
                use std::io::Read as _;
                let mut main = String::new();
                std::io::stdin().read_to_string(&mut main)?;
                temp_script(config, &main, "stdin")?
            } else {
                dunce::canonicalize(PathBuf::from(script))?
            };
            cargo_eval::ops::run(config, &manifest_path, &script_args, release)
                .map_err(|err| to_run_error(config, err))?;
        }
        Action::Clean => {
            let manifest_path = dunce::canonicalize(PathBuf::from(script))?;
            cargo_eval::ops::clean(config, &manifest_path)?;
        }
        Action::Test => {
            let manifest_path = dunce::canonicalize(PathBuf::from(script))?;
            cargo_eval::ops::test(config, &manifest_path)?;
        }
        Action::Bench => {
            let manifest_path = dunce::canonicalize(PathBuf::from(script))?;
            cargo_eval::ops::bench(config, &manifest_path)?;
        }
    }

    Ok(())
}

fn temp_script(config: &cargo::Config, main: &str, id: &str) -> CargoResult<PathBuf> {
    let target_dir = config.target_dir().transpose().unwrap_or_else(|| {
        cargo_eval::config::default_target_dir().map(cargo::util::Filesystem::new)
    })?;
    let hash = blake3::hash(main.as_bytes()).to_string();
    let mut main_path = target_dir.as_path_unlocked().to_owned();
    main_path.push("eval");
    main_path.push(&hash[0..2]);
    main_path.push(&hash[2..4]);
    main_path.push(&hash[4..]);
    std::fs::create_dir_all(&main_path)
        .with_context(|| format!("failed to create temporary main at {}", main_path.display()))?;
    main_path.push(format!("{id}.rs"));
    cargo_eval::util::write_if_changed(&main_path, main)?;
    Ok(main_path)
}

fn to_run_error(config: &cargo::util::Config, err: anyhow::Error) -> CliError {
    let proc_err = match err.downcast_ref::<cargo_util::ProcessError>() {
        Some(e) => e,
        None => return CliError::new(err, 101),
    };

    // If we never actually spawned the process then that sounds pretty
    // bad and we always want to forward that up.
    let exit_code = match proc_err.code {
        Some(exit) => exit,
        None => return CliError::new(err, 101),
    };

    // If `-q` was passed then we suppress extra error information about
    // a failed process, we assume the process itself printed out enough
    // information about why it failed so we don't do so as well
    let is_quiet = config.shell().verbosity() == cargo::core::shell::Verbosity::Quiet;
    if is_quiet {
        CliError::code(exit_code)
    } else {
        CliError::new(err, exit_code)
    }
}

enum Action {
    Run,
    Clean,
    Test,
    Bench,
}

#[test]
fn verify_cli() {
    cli().debug_assert()
}
