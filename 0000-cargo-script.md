- Feature Name: cargo-script
- Start Date: 2023-03-31
- Pre-RFC: [internals](https://internals.rust-lang.org/t/pre-rfc-cargo-script-for-everyone/18639)
- RFC PR: [rust-lang/rfcs#0000](https://github.com/rust-lang/rfcs/pull/0000)
- Rust Issue: [rust-lang/rust#0000](https://github.com/rust-lang/rust/issues/0000)

# Summary
[summary]: #summary

`cargo-eval` is a new program included with rust that can be used for
single-file cargo packages which are `.rs` files with an embedded manifest.
This can be placed in a `#!` line for directly running these files.  The
manifest would be a module-level doc comment with a code fence with `cargo` as
the type.

Example:
```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [dependencies]
//! clap = { version = "4.2", features = ["derive"] }
//! ```

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(short, long, help = "Path to config")]
    config: Option<std::path::PathBuf>,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);
}
```
```console
$ ./prog --config file.toml
Args { config: Some("file.toml") }
```

See [`cargo-script-mvs`](https://crates.io/crates/cargo-script-mvs) for a demo.

# Motivation
[motivation]: #motivation

**Collaboration:**

When sharing reproduction cases, it is much easier when everything exists in a
single code snippet to copy/paste.  Alternatively, people will either leave off
the manifest or underspecify the details of it.

This similarly makes it easier to share code samples with coworkers or in books
/ blogs when teaching.

**Interoperability:**

One angle to look at including something is if there is a single obvious
solution.  While there isn't in the case for `cargo-eval`, there is enough of
a subset of one. By standardizing that subset, we allow greater
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

### Creating a New Package

*(Adapted from [the cargo book](https://doc.rust-lang.org/cargo/guide/creating-a-new-project.html))*

To start a new [package][def-package] with Cargo, create a file named `hello_world.rs`:
```rust
#!/usr/bin/env cargo-eval

fn main() {
    println!("Hello, world!");
}
```

Let's run it
```console
$ chmod +x hello_world.rs
$ ./hello_world.rs
Hello, world!
```

### Dependencies

*(Adapted from [the cargo book](https://doc.rust-lang.org/cargo/guide/dependencies.html))*

[crates.io] is the Rust community's central [*package registry*][def-package-registry]
that serves as a location to discover and download
[packages][def-package]. `cargo` is configured to use it by default to find
requested packages.

#### Adding a dependency

To depend on a library hosted on [crates.io], you modify `hello_world.rs`:
```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [dependencies]
//! time = "0.1.12"
//! ```

fn main() {
    println!("Hello, world!");
}
```

The `cargo` section is called a [***manifest***][def-manifest], and it contains all of the
metadata that Cargo needs to compile your package. This is written in the
[TOML] format (pronounced /tɑməl/).

`time = "0.1.12"` is the name of the [crate][def-crate] and a [SemVer] version
requirement. The [specifying
dependencies](https://doc.rust-lang.org/cargo/guide/../reference/specifying-dependencies.html) docs have more
information about the options you have here.

If we also wanted to add a dependency on the `regex` crate, we would not need
to add `[dependencies]` for each crate listed. Here's what your whole
`hello_world.rs` file would look like with dependencies on the `time` and `regex`
crates:

```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [dependencies]
//! time = "0.1.12"
//! regex = "0.1.41"
//! ```

fn main() {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    println!("Did our date match? {}", re.is_match("2014-01-01"));
}
```

You can then re-run this and Cargo will fetch the new dependencies and all of their dependencies.  You can see this by passing in `--verbose`:
```console
$ cargo eval --verbose ./hello_world.rs
      Updating crates.io index
   Downloading memchr v0.1.5
   Downloading libc v0.1.10
   Downloading regex-syntax v0.2.1
   Downloading memchr v0.1.5
   Downloading aho-corasick v0.3.0
   Downloading regex v0.1.41
     Compiling memchr v0.1.5
     Compiling libc v0.1.10
     Compiling regex-syntax v0.2.1
     Compiling memchr v0.1.5
     Compiling aho-corasick v0.3.0
     Compiling regex v0.1.41
     Compiling hello_world v0.1.0 (file:///path/to/package/hello_world)
Did our date match? true
```

## Package Layout

*(Adapted from [the cargo book](https://doc.rust-lang.org/cargo/guide/project-layout.html))*

When a single file is not enough, you can separately define a `Cargo.toml` file along with the `src/main.rs` file.  Run
```console
$ cargo new hello_world --bin
```

We’re passing `--bin` because we’re making a binary program: if we
were making a library, we’d pass `--lib`. This also initializes a new `git`
repository by default. If you don't want it to do that, pass `--vcs none`.

Let’s check out what Cargo has generated for us:
```console
$ cd hello_world
$ tree .
.
├── Cargo.toml
└── src
    └── main.rs

1 directory, 2 files
```
Unlike the `hello_world.rs`, a little more context is needed in `Cargo.toml`:
```toml
[package]
name = "hello_world"
version = "0.1.0"
edition = "2021"

[dependencies]

```

Cargo uses conventions for file placement to make it easy to dive into a new
Cargo [package][def-package]:

```text
.
├── Cargo.lock
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── main.rs
│   └── bin/
│       ├── named-executable.rs
│       ├── another-executable.rs
│       └── multi-file-executable/
│           ├── main.rs
│           └── some_module.rs
├── benches/
│   ├── large-input.rs
│   └── multi-file-bench/
│       ├── main.rs
│       └── bench_module.rs
├── examples/
│   ├── simple.rs
│   └── multi-file-example/
│       ├── main.rs
│       └── ex_module.rs
└── tests/
    ├── some-integration-tests.rs
    └── multi-file-test/
        ├── main.rs
        └── test_module.rs
```

* `Cargo.toml` and `Cargo.lock` are stored in the root of your package (*package
  root*).
* Source code goes in the `src` directory.
* The default library file is `src/lib.rs`.
* The default executable file is `src/main.rs`.
    * Other executables can be placed in `src/bin/`.
* Benchmarks go in the `benches` directory.
* Examples go in the `examples` directory.
* Integration tests go in the `tests` directory.

If a binary, example, bench, or integration test consists of multiple source
files, place a `main.rs` file along with the extra [*modules*][def-module]
within a subdirectory of the `src/bin`, `examples`, `benches`, or `tests`
directory. The name of the executable will be the directory name.

You can learn more about Rust's module system in [the book][book-modules].

See [Configuring a target] for more details on manually configuring targets.
See [Target auto-discovery] for more information on controlling how Cargo
automatically infers target names.

[book-modules]: https://doc.rust-lang.org/cargo/guide/../../book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html
[Configuring a target]: https://doc.rust-lang.org/cargo/guide/../reference/cargo-targets.html#configuring-a-target
[def-package]:           https://doc.rust-lang.org/cargo/guide/../appendix/glossary.html#package          '"package" (glossary entry)'
[Target auto-discovery]: https://doc.rust-lang.org/cargo/guide/../reference/cargo-targets.html#target-auto-discovery
[TOML]: https://toml.io/
[crates.io]: https://crates.io/
[SemVer]: https://semver.org
[def-crate]:             https://doc.rust-lang.org/cargo/guide/../appendix/glossary.html#crate             '"crate" (glossary entry)'
[def-package]:           https://doc.rust-lang.org/cargo/guide/../appendix/glossary.html#package           '"package" (glossary entry)'
[def-package-registry]:  https://doc.rust-lang.org/cargo/guide/../appendix/glossary.html#package-registry  '"package-registry" (glossary entry)'
[def-manifest]:          https://doc.rust-lang.org/cargo/guide/../appendix/glossary.html#manifest          '"manifest" (glossary entry)'

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

## Single-file packages

In addition to today's multi-file packages (`Cargo.toml` file with other `.rs`
files), we are adding the concept of single-file packages which may contain an
embedded manifest.  There is no required distinguishment for a single-file
`.rs` package from any other `.rs` file.

A single-file package may contain an embedded manifest.  An embedded manifest
is stored using `TOML` in a markdown code-fence with `cargo` at the start of the
infostring inside a target-level doc-comment.  It is an error to have multiple
`cargo` code fences in the target-level doc-comment.  We can relax this later,
either merging the code fences or ignoring later code fences.

Supported forms of embedded manifest are:
``````rust
//! ```cargo
//! ```
``````
``````rust
/*!
```cargo
```
*/
``````
``````rust
/*!
 * ```cargo
 * ```
 */
``````

Inferred / defaulted manifest fields:
- `package.name = <slugified file stem>`
- `package.version = "0.0.0"` to [call attention to this crate being used in unexpected places](https://matklad.github.io/2021/08/22/large-rust-workspaces.html#Smaller-Tips)
- `package.publish = false` to avoid accidental publishes, particularly if we
  later add support for including them in a workspace.
- `package.edition = <current>` to avoid always having to add an embedded
  manifest at the cost of potentially breaking scripts on rust upgrades
  - Warn when `edition` is unspecified.  While with `cargo-eval` this will be
    silenced by default, users wanting stability are also likely to be using
    other commands, like `cargo test` and will see it.

Disallowed manifest fields:
- `[workspace]`, `[lib]`, `[[bin]]`, `[[example]]`, `[[test]]`, `[[bench]]`
- `package.workspace`, `package.build`, `package.links`, `package.autobins`, `package.autoexamples`, `package.autotests`, `package.autobenches`

As the primary role for these files is exploratory programming which has a high
edit-to-run ratio, building should be fast.  Therefore `CARGO_TARGET_DIR` will
be shared between single-file packages to allow reusing intermediate build
artifacts.

A single-file package is accepted by cargo commands as a `--manifest-path`
- This is distinguished by the file extension (`.rs`) and that it is a file.
- This allows running `cargo test --manifest-path single.rs`
- `cargo package` / `cargo publish` will normalize this into a multi-file package
- `cargo add` and `cargo remove` may not support editing embedded manifests initially
- Path-dependencies may not refer to single-file packages at this time (they don't have a `lib` target anyways)

The lockfile for single-file packages will be placed in `CARGO_TARGET_DIR`.  In
the future, when workspaces are supported, that will allow a user to have a
persistent lockfile.

## `cargo-eval`

`cargo-eval` is intended for putting in the `#!` for single-file packages:
```rust
#!/usr/bin/env cargo-eval

fn main() {
    println!("Hello world");
}
```
This command will have the same behavior as running `cargo eval` but is
distinct for wider compatibility (see [Naming](#naming)).

To work with `#!`, `cargo-eval` will accept the single-file package and its
arguments as positional arguments on the command-line.

A user may substitute `-` for the single-file package to read it from the
stdin.  `cargo-eval` will also implicitly read from `stdin` if it is not
interactive:
```console
$ echo 'fn main() { println!("Hello world"); }' | cargo-eval
```

As the primary use case is for exploratory programming, the emphasis will be on
build-time performance, rather than runtime performance
- Debug builds will be the default
- A new profile may be added (or existing profiles tweaked) to optimize build times

Similarly, we will invert the default for `RUST_BACKTRACE`, enabling it by
default, to provide more context to aid in debugging of panics.

To not mix in cargo and user output, `cargo-eval` will run as if with `--quiet` by
default, like with `cabal` in Haskell.  On success, `cargo-eval` will print
nothing while error messages will be shown on failure.  In the future, we can
explore showing progress bars if `stdout` is interactive but they will be
cleared by the time cargo is done.  A single `--verbose` will restore normal
output and subsequent `--verbose`s will act like normal.

Most other flags and behavior will be similar to `cargo run`.

# Drawbacks
[drawbacks]: #drawbacks

At the moment, the doc-comment parsing is brittle, relying on regexes, to
extract it and then requires a heavy dependency (a markdown parser) to get the
code fence.

The implicit content of the manifest will be unclear for users.  We can patch
over this as best we can in documentation but the result won't be ideal.

The `bin.name` assigned to the script included a hash as an implementation
detail of the shared cache (for improving build times).  This makes
programmatic choices off of `argv[0]` not work like normal (e.g. multi-call
binaries).  We could settings
[`argv[0]` on unix-like systems](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.arg0)
but could not find something similar for Windows.

This increases the maintenance and support burden for the cargo team, a team
that is already limited in its availability.

Like with all cargo packages, the `target/` directory grows unbounded.  Some
prior art include a cache GC but that is also to clean up the temp files stored
in other locations (our temp files are inside the `target/` dir and should be
rarer).

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

Guidelines used in design decision making include
- Single-file packages should have a first-class experience
  - Provides a higher quality of experience (doesn't feel like a hack or tacked on)
  - Transferable knowledge, whether experience, stackoverflow answers, etc
  - Easier unassisted migration between single-file and multi-file packages
  - Example implications:
    - Workflows, like running tests, should be the same as multi-file packages rather than being bifurcated
    - Manifest formats should be the same rather than using a specialized schema or data format
- Friction for starting a new single-file package should be minimal
  - Easy to remember, minimal syntax so people are more likely to use it in
    one-off cases, experimental or prototyping use cases without tool assistance
  - Example implications:
    - Embedded manifest is optional which also means we can't require users specifying `edition`
    - See also the implications for first-class experience
    - Workspaces for single-file packages should not be auto-discovered as that
      will break unless the workspaces also owns the single-file package which
      will break workflows for just creating a file anywhere to try out an
      idea.
- Cargo/rustc diagnostics and messages (including `cargo metadata`) should be
  in terms of single-file packages and not any temporary files
  - Easier to understand the messages
  - Provides a higher quality of experience (doesn't feel like a hack or tacked on)
  - Example implications:
    - Most likely, we'll need single-file packages to be understood directly by
      rustc so cargo doesn't have to split out the `.rs` content into a temp
      file that gets passed to cargo which will cause errors to point to the
      wrong file

## Embedded Manifest Format

Considerations for embedded manifest include
- How obvious it is for new users when they see it
- How easy it is for newer users to remember it and type it out
- How machine editable it is for `cargo add` and friends
- Needs to be valid Rust code based on the earlier stated design guidelines

**Option 1: Doc-comment**

```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [package]
//! edition = "2018"
//! ```

fn main() {
}
```

This has the advantage of using existing, familiar syntax both to read and write.

**Option 2: Macro**

```rust
#!/usr/bin/env cargo-eval

cargo! {
[package]
edition = "2018"
}

fn main() {
}
```
- The `cargo` macro would need to come from somewhere (`std`?) which means it is taking on `cargo`-specific knowledge
- A lot of tools/IDEs have problems in dealing with macros
- Free-form rust code makes it harder for cargo to make edits to the manifest

**Option 3: Attribute**

```rust
#!/usr/bin/env cargo-eval

#![cargo(manifest = r#"
[package]
edition = "2018"
"#)]

fn main() {
}
```
- `cargo` could register this attribute or `rustc` could get a generic `metadata` attribute
- I posit that this syntax is more intimidating to read and write for newer users
- Free-form rust code makes it harder for cargo to make edits to the manifest
- As an alternative, `manifest` could a less stringly-typed format but that
  makes it harder for cargo to parse and edit, makes it harder to migrate
  between single and multi-file packages, and makes it harder to transfer
  knowledge and experience

**Option 4: Presentation Streams**

YAML allows several documents to be concatenated together variant
[presentation streams](https://yaml.org/spec/1.2.2/#323-presentation-stream)
which might seem familiar as this is frequently used in static-site generators
for adding frontmatter to pages.
What if we extended Rust's syntax to allow something similar?

```rust
#!/usr/bin/env cargo-eval

fn main() {
}

---Cargo.toml
[package]
edition = "2018"
```
- Easiest for machine parsing and editing
- Flexible for manifest, lockfile, and other content
- Being new syntax, there would be a lot of details to work out, including
  - How to delineate and label documents
  - How to allow escaping to avoid conflicts with content in a documents
  - Potentially an API for accessing the document from within Rust
- Unfamiliar, new syntax, unclear how it will work out for newer users

## `edition`

[The `edition` field controls what variant of cargo and the Rust language to use to interpret everything.](https://doc.rust-lang.org/edition-guide/introduction.html)

A policy on this needs to balance
- Matching the expectation of a reproducible Rust experience
- Users wanting the latest experience, in general
- Boilerplate runs counter to experimentation and prototyping
- There might not be a backing file if we read from `stdin`

**Option 1: Fixed Default**

Multi-file packages default the edition to `2015`, effectively requiring every
project to override it for a modern rust experience.  People are likely to get
this by running `cargo new` and could easily forget it otherwise.
```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [package]
//! edition = "2018"
//! ```

fn main() {
}
```

**Option 2: Latest as Default**

Default to the `edition` for the current `cargo` version, assuming single-file
packages will be transient in nature and users will want the current `edition`.

Longer-lived single-file packages are likely to be used with
- other cargo commands, like `cargo test`, so warning when `edition` is defaulted can raise awareness
- workspaces (future possibility), where `cargo-eval` will implicitly inherit `workspace.edition`

```rust
#!/usr/bin/env cargo-eval

fn main() {
}
```

**Option 3: No default**

It is invalid for an embedded manifest to be missing `edition`, erroring when it is missing.

The minimal single-package file would end up being:
```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [package]
//! edition = "2018"
//! ```

fn main() {
}
```
This dramatically increases the amount of boilerplate to get a single-file package going.

**Option 4: Auto-insert latest**

When the edition is unspecified, we edit the source to contain the latest edition.

```rust
#!/usr/bin/env cargo-eval

fn main() {
}
```
is automatically converted to
```rust
#!/usr/bin/env cargo-eval

//! ```cargo
//! [package]
//! edition = "2018"
//! ```

fn main() {
}
```

This won't work for the `stdin` case.

**Option 5: `cargo-eval --edition <YEAR>`**

Users can do:
```rust
#!/usr/bin/env -S cargo-eval --edition 2018

fn main() {
}
```

The problem is this does not work on all platforms that support `#!`

**Option 6: `carg-eval-<edition>` variants**

Instead of an extra flag, we embed it in the binary name like:
```rust
#!/usr/bin/env -S cargo-eval-2018

fn main() {
}
```
`cargo-eval` would error if both are specified.

On unix-like systems, these could be links to `cargo-eval` and `cargo-eval` can
parse `argv[0]` to extract the `edition`.

However, on Windows the best we can do is a proxy to redirect to `cargo-eval`.

Over the next 40 years, we'll have dozen editions which will bloat the directory, both in terms of the number of files (which can slow things down) and in terms of file size on Windows.

## Scope

The `cargo-script` family of tools has a single command
- Run `.rs` files with embedded manifests
- Evaluate command-line arguments (`--expr`, `--loop`)

This behavior (minus embedded manifests) mirrors what you might expect from a
scripting environment, minus a REPL.  We could design this with the future possibility of a REPL.

However
- The needs of `.rs` files and REPL / CLI args are different, e.g. where they get their dependency definitions
- A REPL is a lot larger of a problem, needing to pull in a lot of interactive behavior that is unrelated to `.rs` files
- A REPL for Rust is a lot more nebulous of a future possibility, making it pre-mature to design for it in mind

Therefore, this RFC proposes we limit the scope of the new command to `cargo run` for single-file rust packages.

## Naming
[naming]: #naming

Considerations:
- The name should tie it back to `cargo` to convey that relationship
- The command run in a `#!` line should not require arguments (e.g. not
  `#!/usr/bin/env cargo <something>`) because it will fail.  `env` treats the
  rest of the line as the bin name, spaces included.  You need to use `env -S`
  but that wasn't supported on macOS at least, last I tested.
- Either don't have a name that looks like a cargo-plugin (e.g. not
  `cargo-<something>`) to avoid confusion or make it work (by default, `cargo
  something` translates to `cargo-something something` which would be ambiguous
  of whether `something` is a script or subcommand)

Candidates
- `cargo-script`:
  - Out of scope
  - Verb preferred
- `cargo-shell`:
  - Out of scope
  - Verb preferred
- `cargo-run`:
  - This would be shorthand for `cargo run --manifest-path <script>.rs`
  - Might be confusing to have slightly different CLI between `cargo-run` and `cargo run`
  - Could add a positional argument to `cargo run` but those are generally avoided in cargo commands
- `cargo-eval`:
  - Currently selected proposal
  - Might convey REPL behavior
  - How do we describe the difference between this and `cargo-run`?
- `cargo-exec`
  - How do we describe the difference between this and `cargo-run`?
- `cargo`:
  - Mirror Haskell's `cabal`
  - Could run into confusion with subcommands but only
    - the script is in the `PATH`
    - the script doesn't have a file extension
    - You are trying to run it as `cargo <script>` (at least on my machine, `#!` invocations canonicalize the file name)
  - Might affect the quality of error messages for invalid subcommands unless we just assume
  - Restricts access to more complex compiler settings unless a user switches
    over to `cargo run` which might have different defaults (e.g. `cargo-eval`
    sets `RUST_BACKTRACE=1`)

## First vs Third Party

As mentioned, a reason for being first-party is to standardize the convention
for this which also allows greater interop.

A default implementation ensures people will use it.  For example, `clap`
received an issue with a reproduction case using a `cargo-play` script that
went unused because it just wasn't worth installing yet another, unknown tool.

This also improves the overall experience as you do not need the third-party
command to replicate support for every potential feature including:
- `cargo test` and other built-in cargo commands
- `cargo expand` and other third-party cargo commands
- `rust-analyzer` and other editor/IDE integration

While other third-party cargo commands might not immediately adopt single-file
packages, first-party support for them will help encourage their adoption.

This still leaves room for third-party implementations, either differentiating themselves or experimenting with
- Alternative caching mechanisms for lower overhead
- Support for implicit `main`, like doc-comment examples
- Template support for implicit `main` for customizing `use`, `extern`, `#[feature]`, etc
- Short-hand dependency syntax (e.g. `//# serde_json = "*"`)
- Prioritizing other workflows, like runtime performance

## File association on Windows

We would add a non-default association to run the file.  We don't want it to be
a default, by default, to avoid unintended harm and due to the likelihood
someone is going to want to edit these files.

## File extension

Should these files use `.rs` or a custom file extension?

Reasons for a unique file type
- Semantics are different than a normal `.rs` file
  - Except already a normal `.rs` file has context-dependent semantics (rest of
    project, `Cargo.toml`, etc), so this doesn't seem too far off
- Different file associations for Windows
- Better detection by tools for the new semantics (particularly `rust-analyzer`)

Downsides to a custom extension
- Limited support by different tools (rust-analyzer, syntax highlighting, non-LSP editor actions) as adoptin rolls out

At this time, we do not see enough reason to use a custom extension when facing the downsides to a slow roll out.

While `rust-analyzer` needs to be able to distinguish regular `.rs` files from
single-file packages to look up the relevant manifest to perform operations, we
propose that be through checking the `#!` line (e.g.
[how perl detects perl in the `#!`](https://stackoverflow.com/questions/38059830/how-does-perl-avoid-shebang-loops).
While this adds boilerplate for Windows developers, this helps encourage
cross-platform development.

If we adopted a unique file extensions, some options include:
- `.crs` (used by `cargo-script`)
- `.ers` (used by `rust-script`)
  - No connection back to cargo
- `.rss`
  - No connection back to cargo
  - Confused with RSS
- `.rsscript`
  - No connection back to cargo
  - Unwieldy
- `.rspkg`
  - No connection back to cargo but conveys its a single-file package

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
  - Implicit main support, including `async main` (different implementation than rustdoc)
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

Java
- [jbang](https://www.jbang.dev/)
  - `jbang init` w/ templates
  - `jbang edit` support, setting up a recommended editor w/ environment
  - Discourages `#!` and instead encourages looking like shell code with `///usr/bin/env jbang "$0" "$@" ; exit $?`
  - Dependencies and compiler flags controlled via comment-directives, including
    - `//DEPS info.picocli:picocli:4.5.0` (gradle-style locators)
      - Can declare one dependency as the source of versions for other dependencies (bom-pom)
    - `//COMPILE_OPTIONS <flags>`
    - `//NATIVE_OPTIONS <flags>`
    - `//RUNTIME_OPTIONS <flags>`
  - Can run code blocks from markdown
  - `--code` flag to execute code on the command-line
  - Accepts scripts from `stdin`

Haskell
- [`runghc` / `runhaskell`](https://downloads.haskell.org/ghc/latest/docs/users_guide/runghc.html)
  - Users can use the file stem (ie leave off the extension) when passing it in
- [cabal's single-file haskel script](https://cabal.readthedocs.io/en/stable/getting-started.html#run-a-single-file-haskell-script)
  - Command is just `cabal`, which could run into weird situations if a file has the same name as a subcommand
  - Manifest is put in a multi-line comment that starts with `cabal:`
  - Scripts are run with `--quiet`, regardless of which invocation is used
  - Documented in their "Getting Started" and then documented further under `cabal run`.
- [`stack script`](https://www.wespiser.com/posts/2020-02-02-Command-Line-Haskell.html)
  - `stack` acts as a shortcut for use in `#!`
  - Delegates resolver information but can be extended on the command-line
  - Command-line flags may be specified in a multi-line comment starting with `stack script`

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

Perl
- [Re-interprets the `#!`](https://stackoverflow.com/questions/38059830/how-does-perl-avoid-shebang-loops)

Ruby
- [`bundler/inline`](https://bundler.io/guides/bundler_in_a_single_file_ruby_script.html)
  - Uses a code-block to define dependencies, making them available for use

Cross-language
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
- Since single-file packages cannot be inferred and require an explicit
  `--manifest-path`, is there an alternative shorthand we should provide, like
  a short-flag for `--manifest-path`?
  - `p` is taken by `--package`
  - `-m`, `-M`, and `-P` are available, but are the meanings clear enough?
- Is there a way we could track what dependency versions have been built in the
  `CARGO_TARGET_DIR` and give preference to resolve to them, if possible.
- `.cargo/config.toml` and rustup-toolchain behavior
  - These are "environment" config files
  - Should `cargo-eval` run like `cargo run` and use the current environment or
    like `cargo install` and use a consistent environment from run-to-run of
    the target?
  - It would be relatively easy to get this with `.cargo/config.toml` but doing
    so for rustup would require a new proxy that understands `cargo-eval`s
    CLI.
  - This would also reduce unnecessary rebuilds when running a personal script
    (from `PATH`) in a project that has an unrelated `.cargo/config.toml`

# Future possibilities
[future-possibilities]: #future-possibilities

## Implicit `main` support

Like with doc-comment examples, we could support an implicit `main`.

Ideally, this would be supported at the language level
- Ensure a unified experience across the playground, `rustdoc`, and `cargo-eval`
- `cargo-eval` can directly run files rather than writing to intermediate files
  - This gets brittle with top-level statements like `extern` (more historical) or bin-level attributes

Behavior can be controlled through editions

## A REPL

See the [REPL exploration](https://github.com/epage/cargo-script-mvs/discussions/102)

In terms of the CLI side of this, we could name this `cargo shell` where it
drops you into an interactive shell within your current package, loading the
existing dependencies (including dev).  This would then be a natural fit to also have a `--eval
<expr>` flag.

Ideally, this repl would also allow the equivalent of `python -i <file>`, not
to run existing code but to make a specific file's API items available for use
to do interactive whitebox testing of private code within a larger project.

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

## Scaling up

We provide a workflow for turning a single-file package into a multi-file package, whether
on either `cargo-eval` or on `cargo-new` / `cargo-init`.  This would help
smooth out the transition when their program has outgrown being in a
single-file.
