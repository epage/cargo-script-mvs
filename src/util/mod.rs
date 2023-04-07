use anyhow::Context as _;

use crate::CargoResult;

pub mod script;

pub fn write_if_changed(path: &std::path::Path, new: &str) -> CargoResult<()> {
    let write_needed = match std::fs::read_to_string(path) {
        Ok(current) => current != new,
        Err(_) => true,
    };
    if write_needed {
        std::fs::write(path, new).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}
