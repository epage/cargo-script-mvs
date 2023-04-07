use crate::CargoResult;

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum UnstableFlags {
    Eval,
    Loop,
    Polyfill,
}

pub fn default_target_dir() -> CargoResult<std::path::PathBuf> {
    let mut cargo_home = home::cargo_home()?;
    cargo_home.push("eval");
    cargo_home.push("target");
    Ok(cargo_home)
}
