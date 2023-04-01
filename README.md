[![CI](https://github.com/fornwall/cargo-eval/workflows/CI/badge.svg)](https://github.com/fornwall/cargo-eval/actions?query=workflow%3ACI)
[![Crates.io](https://img.shields.io/crates/v/cargo-eval.svg)](https://crates.io/crates/cargo-eval)

Note: this is a demo for the corresponding
[RFC](https://github.com/epage/cargo-script-mvs/blob/main/0000-cargo-script.md).
Devitions from the RFC include:
- Not as many compilation flags (e.g. `--profile`)
- `-Zpolyfill` flags like `--test` to demo how `cargo test` might work
- Assuming a "shell" script is actually a parameter from `cargo` and dropping it
- Implementation: Writing an explicit `Cargo.toml` in the target dir since
  cargo does not understand embedded manifests yet

- [Overview](#overview)
- [Installation](#installation)
  - [Distro Packages](#distro-packages)
    - [Arch Linux](#arch-linux)
- [Scripts](#scripts)
- [Executable Scripts](#executable-scripts)
- [Expressions](#expressions)
- [Filters](#filters)
- [Environment Variables](#environment-variables)
- [Troubleshooting](#troubleshooting)

## Overview

Run cargo scripts without any setup or explicit compilation step, with seamless
use of crates specified as dependencies inside the scripts.

```console
$ cargo install cargo-script-mvs
[...]

$ cat script.rs
#!/usr/bin/env cargo-eval
//! Dependencies can be specified in the script file itself as follows:
//!
//! ```cargo
//! [dependencies]
//! rand = "0.8.0"
//! ```

use rand::prelude::*;

fn main() {
    let x: u64 = random();
    println!("A random number: {}", x);
}

$ ./script.rs
A random number: 9240261453149857564
```

With `cargo-eval` Rust files and expressions can be executed just like a shell or Python script. Features include:

- Caching compiled artifacts for speed.
- Reading Cargo manifests embedded in Rust scripts.
- Supporting executable Rust scripts via Unix shebangs
- Using expressions as stream filters (*i.e.* for use in command pipelines).
- Running unit tests and benchmarks from scripts.

You can get an overview of the available options using the `--help` flag.

## Installation

Install or update `cargo-eval` using Cargo:

```console
$ cargo install cargo-script-mvs
```

## Scripts

The primary use for `cargo-eval` is for running Rust source files as scripts. For example:

```console
$ echo 'fn main() {println!("Hello, World!");}' > hello.rs
$ cargo-eval hello.rs
Hello, World!
```

This is the equivalent of:
```console
$ mkdir src
$ echo 'fn main() {println!("Hello, World!");}' > src/hello.rs
$ echo '[package]' > Cargo.toml
$ echo 'name = "hello"' >> Cargo.toml
$ echo 'version = "0.0.0"' >> Cargo.toml
$ cargo run --quiet
```

To show the compilation output, pass `--verbose`.

`cargo-eval` will look for embedded dependency and manifest information in the
script as shown by the below `now.rs` variants:

```rust
#!/usr/bin/env cargo-eval

//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! time = "0.1.25"
//! ```

fn main() {
    println!("{}", time::now().rfc822z());
}
```

## Executable Scripts

On Unix systems, you can use `#!/usr/bin/env cargo-eval` as a shebang line in
a Rust script.  This will allow you to execute a script files (which don't need
to have the `.rs` file extension) directly.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
