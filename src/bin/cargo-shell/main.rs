//! `cargo add`
#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

mod shell;

use std::ffi::OsStr;

fn main() {
    env_logger::init_from_env("CARGO_LOG");
    // HACK: Make this work both as a cargo plugin or not by stripping out the argument cargo
    // passes in to signify which plugin this is.
    let mut args = std::env::args_os().peekable();
    args.next(); // strip binary name
    if args.peek().map(|s| s.as_os_str()) == Some(OsStr::new("shell")) {
        args.next(); // Strip command name, if present
    }

    let cli = shell::cli().no_binary_name(true);
    let matches = cli.get_matches_from(args);

    let mut config = cargo::util::config::Config::default().unwrap_or_else(|e| {
        let mut shell = cargo::core::shell::Shell::new();
        cargo::exit_with_error(e.into(), &mut shell)
    });
    if let Err(e) = shell::exec(&matches, &mut config) {
        cargo::exit_with_error(e, &mut config.shell())
    }
}
