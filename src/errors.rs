/// Common result type
pub type CargoResult<T> = anyhow::Result<T>;

/// Common error type
pub type Error = anyhow::Error;

pub use anyhow::Context;

/// CLI-specific result
pub type CliResult = Result<(), CliError>;

pub use cargo::util::errors::CliError;
