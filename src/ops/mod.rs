use crate::CargoResult;
use crate::CliResult;

pub fn run(
    config: &cargo::Config,
    manifest_path: &std::path::Path,
    args: &[std::ffi::OsString],
    release: bool,
) -> CargoResult<()> {
    let script = crate::util::script::RawScript::parse_from(manifest_path)?;
    let ws = script.to_workspace(config)?;

    let mut build_config = cargo::core::compiler::BuildConfig::new(
        config,
        None,
        false,
        &[],
        cargo::core::compiler::CompileMode::Build,
    )?;
    build_config.requested_profile =
        cargo::util::interning::InternedString::new(if release { "release" } else { "dev" });
    let compile_opts = cargo::ops::CompileOptions {
        build_config,
        cli_features: cargo::core::resolver::features::CliFeatures::from_command_line(
            &[],
            false,
            true,
        )?,
        spec: cargo::ops::Packages::Default,
        filter: cargo::ops::CompileFilter::Default {
            required_features_filterable: false,
        },
        target_rustdoc_args: None,
        target_rustc_args: None,
        target_rustc_crate_types: None,
        rustdoc_document_private_items: false,
        honor_rust_version: true,
    };

    cargo::ops::run(&ws, &compile_opts, args)
}

pub fn clean(config: &cargo::Config, manifest_path: &std::path::Path) -> CargoResult<()> {
    let opts = cargo::ops::CleanOptions {
        config,
        spec: vec![],
        targets: vec![],
        requested_profile: cargo::util::interning::InternedString::new("dev"),
        profile_specified: false,
        doc: false,
    };
    let script = crate::util::script::RawScript::parse_from(manifest_path)?;
    let ws = script.to_workspace(config)?;
    cargo::ops::clean(&ws, &opts)?;
    Ok(())
}

pub fn test(config: &cargo::Config, manifest_path: &std::path::Path) -> CliResult {
    let script = crate::util::script::RawScript::parse_from(manifest_path)?;
    let ws = script.to_workspace(config)?;

    let mut build_config = cargo::core::compiler::BuildConfig::new(
        config,
        None,
        false,
        &[],
        cargo::core::compiler::CompileMode::Test,
    )?;
    build_config.requested_profile = cargo::util::interning::InternedString::new("test");
    let compile_opts = cargo::ops::CompileOptions {
        build_config,
        cli_features: cargo::core::resolver::features::CliFeatures::from_command_line(
            &[],
            false,
            true,
        )?,
        spec: cargo::ops::Packages::Default,
        filter: cargo::ops::CompileFilter::from_raw_arguments(
            false,
            vec![],
            true,
            vec![],
            false,
            vec![],
            false,
            vec![],
            false,
            false,
        ),
        target_rustdoc_args: None,
        target_rustc_args: None,
        target_rustc_crate_types: None,
        rustdoc_document_private_items: false,
        honor_rust_version: true,
    };

    let ops = cargo::ops::TestOptions {
        no_run: false,
        no_fail_fast: false,
        compile_opts,
    };

    cargo::ops::run_tests(&ws, &ops, &[])
}

pub fn bench(config: &cargo::Config, manifest_path: &std::path::Path) -> CliResult {
    let script = crate::util::script::RawScript::parse_from(manifest_path)?;
    let ws = script.to_workspace(config)?;

    let mut build_config = cargo::core::compiler::BuildConfig::new(
        config,
        None,
        false,
        &[],
        cargo::core::compiler::CompileMode::Bench,
    )?;
    build_config.requested_profile = cargo::util::interning::InternedString::new("bench");
    let compile_opts = cargo::ops::CompileOptions {
        build_config,
        cli_features: cargo::core::resolver::features::CliFeatures::from_command_line(
            &[],
            false,
            true,
        )?,
        spec: cargo::ops::Packages::Default,
        filter: cargo::ops::CompileFilter::from_raw_arguments(
            false,
            vec![],
            true,
            vec![],
            false,
            vec![],
            false,
            vec![],
            false,
            false,
        ),
        target_rustdoc_args: None,
        target_rustc_args: None,
        target_rustc_crate_types: None,
        rustdoc_document_private_items: false,
        honor_rust_version: true,
    };

    let ops = cargo::ops::TestOptions {
        no_run: false,
        no_fail_fast: false,
        compile_opts,
    };

    cargo::ops::run_benches(&ws, &ops, &[])
}
