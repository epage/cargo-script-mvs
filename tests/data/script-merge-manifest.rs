//! This is merged into a default manifest in order to form the full package manifest:
//!
//! ```cargo
//! [package]
//! name = "TEST-script-merge-manifest"
//! version = "0.1.3"
//! authors = ["mna king", "squeeze@merge.com"]
//! [bin]
//! name="xyz"
//! path="pdq.rs"
//! [dependencies]
//! boolinator = "=0.1.0"
//! tokio = { version = "1", features = ["full"] }
//! ```
use boolinator::Boolinator;

#[tokio::main]
async fn main() {
    println!("--output--");
    let rsname = option_env!("CARGO_PKG_NAME").unwrap_or("unknown");
    let authors = option_env!("CARGO_PKG_AUTHORS").unwrap_or("unknown");
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let cargo_basedir = option_env!("CARGO_BUILD_DEP_INFO_BASEDIR").unwrap_or("unknown");
    println!("Name = {}", rsname);
    println!("Authors = {}", authors);
    println!("Version = {}", version);
    println!("Cargo basdir = {}", cargo_basedir);

    println!("{:?}", true.as_some(1));
}
