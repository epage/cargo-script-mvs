#!/usr/bin/env cargo-shell

//! ```cargo
//! [dependencies]
//! path_dep.path = "path_dep"
//! ```

fn main() {
    let message = path_dep::message();
    println!("{}", message);
}
