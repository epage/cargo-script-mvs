- Feature Name: cargo-shell
- Start Date: 2023-03-31
- RFC PR: [rust-lang/rfcs#0000](https://github.com/rust-lang/rfcs/pull/0000)
- Rust Issue: [rust-lang/rust#0000](https://github.com/rust-lang/rust/issues/0000)

# Summary
[summary]: #summary

`cargo-shell` is a new program included with rust that can be used for
single-file cargo packages which are `.rs` files with an embedded manifest.
This can be placed in a `#!` line for directly running these files.  The
manifest would be a module-level doc comment with a code fence with `cargo` as
the type.

# Motivation
[motivation]: #motivation

**Collaboration:**

When sharing reproduction cases, it is much easier when everything exists in a
single code snippet to copy/paste.  Alternatively, people will either leave off
the manifest or underspecify the details of it.

This similarly makes it easier to share code samples with coworkers or in books
/ blogs.

**Interoperability:**

One angle to look at including something is if there is a single obvious
solution.  While there isn't in the case for `cargo-shell`, there is enough of
a subset of one that by standardizing that subset, we allow greater
interoperability between solutions (e.g.
[playground could gain support](https://users.rust-lang.org/t/call-for-contributors-to-the-rust-playground-for-upcoming-features/87110/14?u=epage)
).  This would make it easier to collaborate..

**Prototyping:**

Currently to prototype or try experiment with APIs or the language, you need to either
- Use the playground
  - Can't access local resources
  - Limited in the crates supported
  - *Note:* there are alternatives to the playground that might have fewer
    restrictions but are either less well known or have additional
    complexities.
- Find a place to do `cargo new`, edit `Cargo.toml` and `main.rs` as necessary, and `cargo run` it, then delete it
  - This is a lot of extra steps, increasing the friction to trying things out
  - This will fail if you create in a place that `cargo` will think it should be a workspace member

By having a single-file project,
- It is easier to setup and tear down these experiments, making it more likely to happen
- All crates will be available
- Local resources are available

**One-Off Utilities:**

It is fairly trivial to create a bunch of single-file bash or python scripts
into a directory and add it to the path.  Compare this to rust where
- `cargo new` each of the "scripts" into individual directories
- Create wrappers for each so you can access it in your path, passing `--manifest-path` to `cargo run`

# Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

This will work like any other cargo command:
- It will sit below `rustup` which means it will respect the rust toolchain file
- It will respect the `.cargo/config.toml` from the CWD

# Drawbacks
[drawbacks]: #drawbacks

This increases the maintenance and support burden for the cargo team, a team
that is already limited in its availability.

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

Misc
- Use `package.rust-version` to control the toolchain
  - Why not: this will be sitting below rustup, not above it

## Naming

By using `rust` in the name, it makes it sound like its parallel or below
cargo, rather than integrates with cargo support.

When naming it `cargo <something>`, `#!/usr/bin/env cargo <something>` will
fail because `env` treats the rest of the line as the bin name, spaces
included.  You need to use `env -S` but that wasn't supported at least on macOS
last I tested.

When naming `cargo-<something>` (e.g. `cargo-script`), we are following the
convention of a cargo plugin and users have full right to expect it to work but
it will fail because cargo will run it as
`cargo-<something> <something>`.

# Prior art
[prior-art]: #prior-art

Rust, same space
- [`cargo-script`](https://github.com/DanielKeep/cargo-script)
  - Single-file (`.crs` extension) rust code
    - Partial manifests in a `cargo` doc comment code fence or dependencies in a comment directive
    - `run-cargo-script` for she-bangs and setting up file associations on Windows
  - Performance: Shares a `CARGO_TARGET_DIR`, reusing dependency builds
  - `--expr <expr>` for expressions as args (wraps in a block and prints blocks value as `{:?}` )
     - `--dep` flags since directives don't work as easily
  - `--loop <expr>` for a closure to run on each line
  - `--test`, etc flags to make up for cargo not understanding thesefiles
  - `--force` to rebuild` and `--clear-cache`
  - Communicates through scrpts through some env variables
- [`cargo-scripter`](https://crates.io/crates/cargo-scripter)
  - See above with 8 more commits
- [`cargo-eval`](https://crates.io/crates/cargo-eval)
  - See above with a couple more commits
- [`rust-script`](https://crates.io/crates/rust-script)
  - See above
  - Changed extension to `.ers` / `.rs`
  - Single binary without subcommands in primary case for ease of running
  - Inferred-main support, including `async main` (different implementation than rustdoc)
  - `--toolchain-version` flag
- [`cargo-play`](https://crates.io/crates/cargo-play)
  - Allows multiple-file scripts, first specified is the `main`
  - Dependency syntax `//# serde_json = "*"`
  - Otherwise, seems like it has a subset of `cargo-script`s functionality
- [`cargo-wop`](https://crates.io/crates/cargo-wop)
  - `cargo wop` is to single-file rust scripts as `cargo` is to multi-file rust projects
  - Dependency syntax is a doc comment code fence

Rust, related space
- [Playground](https://play.rust-lang.org/)
  - Includes top 100 crates
- [Rust Explorer](https://users.rust-lang.org/t/rust-playground-with-the-top-10k-crates/75746)
  - Uses a comment syntax for specifying dependencies
- [`runner`](https://github.com/stevedonovan/runner/)
  - Global `Cargo.toml` with dependencies added via `runner --add <dep>` and various commands  / args to interact with the shared crate
  - Global, editable prelude / template
  - `-e <expr>` support
  - `-i <expr>` support for consuming and printing iterator values
  - `-n <expr>` runs per line
- [`evcxr`](https://github.com/google/evcxr)
  - Umbrella project which includes a REPL and Jupyter kernel
  - Requires opting in to not ending on panics
  - Expressions starting with `:` are repl commands
  - Limitations on using references
- [`irust`](https://github.com/sigmaSd/IRust)
  - Rust repl
  - Expressions starting with `:` are repl commands
  - Global, user-editable prelude crate
- [papyrust](https://crates.io/crates/papyrust)
  - Not single file; just gives fast caching for a cargo package

D
- [rdmd](https://dlang.org/rdmd.html)
  - More like `rustc`, doesn't support package-manager dependencies?
  - `--eval=<code>` flag
  - `--loop=<code>` flag
  - `--force` to rebuild
  - `--main` for adding an empty `main`, e.g. when running a file with tests

Bash
- `bash` to get an interactive way of entering code
- `bash file` will run the code in `file,` searching in `PATH` if it isn't available locally
- `./file` with `#!/usr/bin/env bash` to make standalone executables
- `bash -c <expr>` to try out an idea right now
- Common configuration with rc files, `--rcfile <path>`

Python
- `python` to get an interactive way of entering code
- `python -i ...` to make other ways or running interactive
- `python <file>` will run the file
- `./file` with `#!/usr/bin/env python` to make standalone executables
- `python -c <expr>` to try out an idea right now
- Can run any file in a project (they can have their own "main") to do whitebox exploratory programming and not just blackblox

Go
- [`gorun`](https://github.com/erning/gorun/) attempts to bring that experience to a compiled language, go in this case
  - `gorun <file>` to build and run a file
  - Implicit garbage collection for build cache
  - Project metadata is specified in HEREDOCs in comments

Generic
- [`scriptisto`](https://github.com/igor-petruk/scriptisto)
  - Supports any compiled language
  - Comment-directives give build commands
- [nix-script](https://github.com/BrianHicks/nix-script)
  - Nix version of scriptisto, letting you use any Nix dependency

# Unresolved questions
[unresolved-questions]: #unresolved-questions

- Can we have both script stability and make it easy to be on the latest edition?
- Could somehow "lock" to what is currently in the shared script cache to avoid
  each script getting the latest version of a crate, causing churn in `target/`?
- Is there a way we can allow whitebox exploratory programming like Python
  (mostly) does where you can run any script within a project?
  - The limitation in Python is on whether your environment and package are
    setup just right for all imports to work

# Future possibilities
[future-possibilities]: #future-possibilities

## CLI Expression Evaluation

Support a `-e` / `--eval` / `--expr` flag that changes the interpretation of the path
parameter to being an expression to evaluate that prints the debug
representation of the result if it isn't `()`.

## A REPL

See the [REPL exploration](https://github.com/epage/cargo-script-mvs/discussions/102)

## Workspace Support

Allow scripts to be members of a workspace.

The assumption is that this will be opt-in, rather than implicit, so you can
easily drop one of these scripts anywhere without it failing because the
workspace root and the script don't agree on workspace membership.  To do this,
we'd expand `package.workspace` to also be a `bool` to control whether a
workspace lookup is disallowed or whether to auto-detect the workspace
- For `Cargo.toml`, `package.workspace = true` is the default
- For cargo-script, `package.workspace = false` is the default

When a workspace is specified
- Use its target directory
- Use its lock file
- Be treated as any other workspace member for `cargo <cmd> --workspace`
- Check what `workspace.package` fields exist and automatically apply them over default manifest fields
- Explicitly require `workspace.dependencies` to be inherited
  - I would be tempted to auto-inherit them but then `cargo rm`s gc will remove them because there is no way to know they are in use
- Apply all `profile` and `patch` settings

This could serve as an alternative to
[`cargo xtask`](https://github.com/matklad/cargo-xtask) with scripts sharing
the lockfile and `target/` directory.
