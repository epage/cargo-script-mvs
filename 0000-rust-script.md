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

**Collaboration:**

When sharing reproduction cases, it is much easier when everything exists in a
single code snippet to copy/paste.  Alternatively, people will either leave off
the manifest or underspecify the details of it.

This similarly makes it easier to share code samples with coworkers or in books
/ blogs.

**One-Off Utilities:**

It is fairly trivial to create a bunch of single-file bash or python scripts
into a directory and add it to the path than it is to `cargo new` a bunch of
cargo packages and then create bash wrappers within the path to then call those
script

# Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

This will work like any other cargo command:
- It will sit below `rustup` which means it will respect the rust toolchain file
- It will respect the `.cargo/config.toml` from the CWD

# Drawbacks
[drawbacks]: #drawbacks

Why should we *not* do this?

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

Misc
- Use `package.rust-version` to control the toolchain
  - Why not: this will be sitting below rustup, not above it

# Prior art
[prior-art]: #prior-art


# Unresolved questions
[unresolved-questions]: #unresolved-questions

- Can we have both script stability and make it easy to be on the latest edition?
- Could somehow "lock" to what is currently in the shared script cache to avoid
  each script getting the latest version of a crate, causing churn in `target/`?

# Future possibilities
[future-possibilities]: #future-possibilities

## Workspace Support

Allow scripts to be members of a workspace.

The assumption is that this will be opt-in, rather than implicit, so you can
easily drop one of these scripts anywhere without it failing because the
workspace root and the script don't agree on workspace membership.  To do this,
we'd xpand `package.workspace` to also be a `bool` to control whether a
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
