use anyhow::Context as _;

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

pub fn eval(config: &cargo::Config, script: &str, release: bool) -> CargoResult<()> {
    let main = EVAL_TEMPLATE.replace("#{script}", script);
    let main_path = temp_script(config, &main, "eval")?;
    run(config, &main_path, &[], release)
}

const EVAL_TEMPLATE: &str = r#"
#![allow(unreachable_code)]
use std::any::{Any, TypeId};
fn main() {
    let exit_code = match try_main() {
        Ok(()) => None,
        Err(e) => {
            use std::io::{self, Write};
            let _ = writeln!(io::stderr(), "Error: {}", e);
            Some(1)
        },
    };
    if let Some(exit_code) = exit_code {
        std::process::exit(exit_code);
    }
}
fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    fn _rust_script_is_empty_tuple<T: ?Sized + Any>(_s: &T) -> bool {
        TypeId::of::<()>() == TypeId::of::<T>()
    }
    match {#{script}} {
        __rust_script_expr if !_rust_script_is_empty_tuple(&__rust_script_expr) => println!("{:?}", __rust_script_expr),
        _ => {}
    }
    Ok(())
}
"#;

pub fn loop_(config: &cargo::Config, script: &str, release: bool) -> CargoResult<()> {
    let main = LOOP_TEMPLATE.replace("#{script}", script);
    let main_path = temp_script(config, &main, "loop")?;
    run(config, &main_path, &[], release)
}

const LOOP_TEMPLATE: &str = r#"
#![allow(unused_imports)]
#![allow(unused_braces)]
use std::any::Any;
use std::io::prelude::*;
fn main() {
    let mut closure = enforce_closure(
{#{script}}
    );
    let mut line_buffer = String::new();
    let stdin = std::io::stdin();
    loop {
        line_buffer.clear();
        let read_res = stdin.read_line(&mut line_buffer).unwrap_or(0);
        if read_res == 0 { break }
        let output = closure(&line_buffer);
        let display = {
            let output_any: &dyn Any = &output;
            !output_any.is::<()>()
        };
        if display {
            println!("{:?}", output);
        }
    }
}
fn enforce_closure<F, T>(closure: F) -> F
where F: FnMut(&str) -> T, T: 'static {
    closure
}
"#;

fn temp_script(config: &cargo::Config, main: &str, id: &str) -> CargoResult<std::path::PathBuf> {
    let target_dir = config
        .target_dir()
        .transpose()
        .unwrap_or_else(|| crate::config::default_target_dir().map(cargo::util::Filesystem::new))?;
    let hash = blake3::hash(main.as_bytes()).to_string();
    let mut main_path = target_dir.as_path_unlocked().to_owned();
    main_path.push("shell");
    main_path.push(&hash[0..2]);
    main_path.push(&hash[2..4]);
    main_path.push(&hash[4..]);
    std::fs::create_dir_all(&main_path)
        .with_context(|| format!("failed to create temporary main at {}", main_path.display()))?;
    main_path.push(format!("{id}.rs"));
    crate::util::write_if_changed(&main_path, main)?;
    Ok(main_path)
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
