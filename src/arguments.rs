use std::ffi::OsString;
use std::path::PathBuf;

use crate::build_kind::BuildKind;

#[derive(Debug)]
pub struct Args {
    pub script: Option<OsString>,
    pub script_args: Vec<OsString>,
    pub features: Vec<String>,

    pub expr: bool,

    pub pkg_path: Option<PathBuf>,
    pub cargo_output: bool,
    pub clear_cache: bool,
    pub debug: bool,
    pub force: bool,
    pub build_kind: BuildKind,
    // This is a String instead of an enum since one can have custom
    // toolchains (ex. a rustc developer will probably have `stage1`):
    pub toolchain_version: Option<String>,

    #[cfg(windows)]
    pub install_file_association: bool,
    #[cfg(windows)]
    pub uninstall_file_association: bool,

    #[allow(dead_code)]
    pub unstable_flags: Vec<UnstableFlags>,
}

impl Args {
    pub fn parse() -> anyhow::Result<Self> {
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
                        "install-file-association",
                        "uninstall-file-association",
                    ]
                } else {
                    vec!["clear-cache"]
                })
                .conflicts_with_all(if cfg!(windows) {
                    vec!["install-file-association", "uninstall-file-association"]
                } else {
                    vec![]
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
            Arg::new("toolchain-version")
                .help("Build the script using the given toolchain version (unstable)")
                .long("toolchain-version")
                // "channel"
                .short('c')
                // FIXME: remove if benchmarking is stabilized
                .conflicts_with("bench"),
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
                    .help(
                        "Uninstall the file association that makes rust-script execute .ers files",
                    )
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
                    anyhow::bail!(
                        "`--test` is unstable and requires `-Z test` (epage/cargo-script-mvs#29)."
                    )
                }
            }
            BuildKind::Bench => {
                if !unstable_flags.contains(&UnstableFlags::Bench) {
                    anyhow::bail!(
                    "`--bench` is unstable and requires `-Z bench` (epage/cargo-script-mvs#68)."
                )
                }
            }
        }

        let toolchain_version = m.get_one::<String>("toolchain-version").map(Into::into);
        if let Some(toolchain_version) = &toolchain_version {
            if !unstable_flags.contains(&UnstableFlags::ToolchainVersion) {
                anyhow::bail!("`--toolchain-version={toolchain_version}` is unstable and requires `-Z toolchain-version` (epage/cargo-script-mvs#36).")
            }
        }

        let expr = *m.get_one::<bool>("expr").expect("defaulted");
        if expr && !unstable_flags.contains(&UnstableFlags::Expr) {
            anyhow::bail!(
                "`--expr` is unstable and requires `-Z expr` (epage/cargo-script-mvs#72)."
            )
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum UnstableFlags {
    Test,
    Bench,
    ToolchainVersion,
    Expr,
}
