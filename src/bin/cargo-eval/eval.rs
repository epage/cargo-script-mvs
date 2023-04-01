use std::ffi::OsString;
use std::path::PathBuf;

use cargo::util::errors::CliError;

use cargo_eval::config::UnstableFlags;
use cargo_eval::CliResult;

pub fn cli() -> clap::Command {
    clap::command!()
        .args([
            clap::Arg::new("script")
                .num_args(1..)
                .value_names(["script", "arg"])
                .trailing_var_arg(true)
                .required(true)
                .value_parser(clap::value_parser!(OsString))
                .help("Script file or expression to execute"),
            clap::Arg::new("eval")
                .short('e')
                .long("eval")
                .action(clap::ArgAction::SetTrue)
                .help("Run `<script>` as a literal expression and display the result (unstable)")
                .requires("script"),
            clap::Arg::new("loop")
                .short('l')
                .long("loop")
                .action(clap::ArgAction::SetTrue)
                .help(
                    "Run `<script>` as a literal closure once for each line from stdin (unstable)",
                )
                .requires("script"),
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
                .help("Remove the script target director (unstable)")
                .help_heading("Pollyfill")
                .action(clap::ArgAction::SetTrue)
                .requires("script")
                .group("action"),
            clap::Arg::new("test")
                .long("test")
                .action(clap::ArgAction::SetTrue)
                .help("Compile and run tests (unstable)")
                .help_heading("Pollyfill")
                .requires("script")
                .group("action"),
            clap::Arg::new("bench")
                .long("bench")
                .action(clap::ArgAction::SetTrue)
                .help("Compile and run benchmarks (unstable)")
                .help_heading("Pollyfill")
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
                anyhow::format_err!("`--clean` is unstable and requires `-Zpollyfill`").into(),
            );
        }
        Action::Clean
    } else if matches.get_flag("test") {
        if !unstable_flags.contains(&UnstableFlags::Polyfill) {
            return Err(
                anyhow::format_err!("`--test` is unstable and requires `-Zpollyfill`").into(),
            );
        }
        Action::Test
    } else if matches.get_flag("bench") {
        if !unstable_flags.contains(&UnstableFlags::Polyfill) {
            return Err(
                anyhow::format_err!("`--bench` is unstable and requires `-Zpollyfill`").into(),
            );
        }
        Action::Bench
    } else if matches.get_flag("eval") {
        if !unstable_flags.contains(&UnstableFlags::Eval) {
            return Err(anyhow::format_err!("`--eval` is unstable and requires `-Zeval`").into());
        }
        Action::Eval
    } else if matches.get_flag("loop") {
        if !unstable_flags.contains(&UnstableFlags::Loop) {
            return Err(anyhow::format_err!("`--loop` is unstable and requires `-Zloop`").into());
        }
        Action::Loop
    } else {
        Action::Run
    };

    let mut script_and_args = matches
        .get_many::<OsString>("script")
        .expect("clap forces `script` to be present")
        .cloned();
    let script = script_and_args
        .next()
        .expect("clap forces `script` to be present");
    let script_args: Vec<OsString> = script_and_args.collect();

    let release = matches.get_flag("release");

    let verbose = matches.get_count("verbose");
    let (verbose, quiet) = if matches!(&action, Action::Run | Action::Eval | Action::Loop) {
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
            let manifest_path = dunce::canonicalize(PathBuf::from(script))?;
            cargo_eval::ops::run(config, &manifest_path, &script_args, release)
                .map_err(|err| to_run_error(config, err))?;
        }
        Action::Eval => {
            if std::env::var_os("RUST_BACKTRACE").is_none() {
                std::env::set_var("RUST_BACKTRACE", "1");
            }
            let script = script.to_str().ok_or_else(|| {
                anyhow::format_err!(
                    "`--eval {}` contains invalid UTF-8",
                    script.to_string_lossy()
                )
            })?;
            cargo_eval::ops::eval(config, script, release)
                .map_err(|err| to_run_error(config, err))?;
        }
        Action::Loop => {
            if std::env::var_os("RUST_BACKTRACE").is_none() {
                std::env::set_var("RUST_BACKTRACE", "1");
            }
            let script = script.to_str().ok_or_else(|| {
                anyhow::format_err!(
                    "`--loop {}` contains invalid UTF-8",
                    script.to_string_lossy()
                )
            })?;
            cargo_eval::ops::loop_(config, script, release)
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
    Eval,
    Loop,
    Clean,
    Test,
    Bench,
}

#[test]
fn verify_cli() {
    cli().debug_assert()
}
