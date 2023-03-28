//! Platform-specific stuff.

use std::path::PathBuf;

#[cfg(not(test))]
pub fn cache_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("RUST_SCRIPT_CACHE_PATH") {
        Ok(path.into())
    } else {
        dirs_next::cache_dir()
            .map(|dir| dir.join(env!("CARGO_PKG_NAME")))
            .ok_or_else(|| anyhow::format_err!("Cannot get cache directory"))
    }
}

#[cfg(test)]
pub fn cache_dir() -> anyhow::Result<PathBuf> {
    static TEMP_DIR: once_cell::sync::Lazy<tempfile::TempDir> =
        once_cell::sync::Lazy::new(|| tempfile::TempDir::new().unwrap());
    Ok(TEMP_DIR.path().to_path_buf())
}

pub fn generated_projects_cache_path() -> anyhow::Result<PathBuf> {
    cache_dir().map(|dir| dir.join("projects"))
}

pub fn binary_cache_path() -> anyhow::Result<PathBuf> {
    cache_dir().map(|dir| dir.join("binaries"))
}
