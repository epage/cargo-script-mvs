/*!
This module is concerned with how `rust-script` extracts the manifest from a script file.
*/
use regex;

use self::regex::Regex;
use std::collections::HashMap;
use std::path::Path;

use crate::consts;
use crate::error::{MainError, MainResult};
use crate::templates;
use crate::Input;
use log::{error, info};
use std::ffi::OsString;

static RE_MARGIN: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^\s*\*( |$)").unwrap());
static RE_SPACE: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^(\s+)").unwrap());
static RE_NESTING: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"/\*|\*/").unwrap());
static RE_COMMENT: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^\s*//(!|/)").unwrap());
static RE_SHEBANG: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^#![^\[].*?(\r\n|\n)").unwrap());
static RE_CRATE_COMMENT: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
    Regex::new(
            r"(?x)
                # We need to find the first `/*!` or `//!` that *isn't* preceded by something that would make it apply to anything other than the crate itself.  Because we can't do this accurately, we'll just require that the doc comment is the *first* thing in the file (after the optional shebang, which should already have been stripped).
                ^\s*
                (/\*!|//(!|/))
            "
        ).unwrap()
});

/**
Splits input into a complete Cargo manifest and unadultered Rust source.

Unless we have prelude items to inject, in which case it will be *slightly* adulterated.
*/
pub fn split_input(input: &Input, input_id: &OsString) -> MainResult<(String, String)> {
    fn contains_main_method(line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("fn main(")
            || line.starts_with("pub fn main(")
            || line.starts_with("async fn main(")
            || line.starts_with("pub async fn main(")
    }

    let template_buf;
    let (part_mani, source, template) = match input {
        Input::File(_, _, content, _) => {
            let content = strip_shebang(content.as_str());
            let (manifest, source) =
                find_embedded_manifest(content).unwrap_or((Manifest::Toml(""), content));

            let source = if source.lines().any(contains_main_method) {
                source.to_string()
            } else {
                format!("fn main() -> Result<(), Box<dyn std::error::Error+Sync+Send>> {{\n    {{\n    {}    }}\n    Ok(())\n}}", source)
            };
            (manifest, source, templates::get_template("file")?)
        }
        Input::Expr(content, template) => {
            template_buf = templates::get_template(template.as_deref().unwrap_or("expr"))?;
            let (manifest, template_src) = find_embedded_manifest(&template_buf)
                .unwrap_or((Manifest::Toml(""), &template_buf));
            (manifest, content.to_string(), template_src.into())
        }
    };

    let mut subs = HashMap::with_capacity(2);

    subs.insert(consts::SCRIPT_BODY_SUB, &source[..]);

    let source = templates::expand(&template, &subs)?;

    info!("part_mani: {:?}", part_mani);
    info!("source: {:?}", source);

    let part_mani = part_mani.into_toml()?;
    info!("part_mani: {:?}", part_mani);

    // It's-a mergin' time!
    let def_mani = default_manifest(input, input_id)?;
    let mani = merge_manifest(def_mani, part_mani)?;

    // Fix up relative paths.
    let mani = fix_manifest_paths(mani, &input.base_path())?;
    info!("mani: {:?}", mani);

    let mani_str = format!("{}", toml::Value::Table(mani));
    info!("mani_str: {}", mani_str);

    Ok((mani_str, source))
}

#[test]
fn test_split_input() {
    let input_id = OsString::from("input_id");
    macro_rules! si {
        ($i:expr) => {
            split_input(&$i, &input_id).ok()
        };
    }

    let f = |c: &str| {
        let dummy_path: std::path::PathBuf = "p".into();
        Input::File("n".into(), dummy_path, c.into(), 0)
    };

    macro_rules! r {
        ($m:expr, $r:expr) => {
            Some(($m.into(), $r.into()))
        };
    }

    assert_eq!(
        si!(f(r#"fn main() {}"#)),
        r!(
            r#"[[bin]]
name = "n_input_id"
path = "n.rs"

[package]
authors = ["Anonymous"]
edition = "2018"
name = "n"
version = "0.1.0"
"#,
            r#"fn main() {}"#
        )
    );

    // Ensure removed prefix manifests don't work.
    assert_eq!(
        si!(f(r#"
---
fn main() {}
"#)),
        r!(
            r#"[[bin]]
name = "n_input_id"
path = "n.rs"

[package]
authors = ["Anonymous"]
edition = "2018"
name = "n"
version = "0.1.0"
"#,
            r#"
---
fn main() {}
"#
        )
    );

    assert_eq!(
        si!(f(r#"[dependencies]
time="0.1.25"
---
fn main() {}
"#)),
        r!(
            r#"[[bin]]
name = "n_input_id"
path = "n.rs"

[package]
authors = ["Anonymous"]
edition = "2018"
name = "n"
version = "0.1.0"
"#,
            r#"[dependencies]
time="0.1.25"
---
fn main() {}
"#
        )
    );

    assert_eq!(
        si!(f(r#"
/*!
Here is a manifest:

```cargo
[dependencies]
time = "0.1.25"
```
*/
fn main() {}
"#)),
        r!(
            r#"[[bin]]
name = "n_input_id"
path = "n.rs"

[dependencies]
time = "0.1.25"

[package]
authors = ["Anonymous"]
edition = "2018"
name = "n"
version = "0.1.0"
"#,
            r#"
/*!
Here is a manifest:

```cargo
[dependencies]
time = "0.1.25"
```
*/
fn main() {}
"#
        )
    );
}

/**
Returns a slice of the input string with the leading shebang, if there is one, omitted.
*/
fn strip_shebang(s: &str) -> &str {
    match RE_SHEBANG.find(s) {
        Some(m) => &s[m.end() - 1..], // subtract 1 to include \n or \r\n that would otherwise have been stripped
        None => s,
    }
}

#[test]
fn test_strip_shebang() {
    assert_eq!(
        strip_shebang(
            "\
#!/usr/bin/env rust-script
and the rest
\
        "
        ),
        "\
\nand the rest
\
        "
    );
    assert_eq!(
        strip_shebang(
            "\
#![thingy]
and the rest
\
        "
        ),
        "\
#![thingy]
and the rest
\
        "
    );
}

/**
Represents the kind, and content of, an embedded manifest.
*/
#[derive(Debug, Eq, PartialEq)]
enum Manifest<'s> {
    /// The manifest is a valid TOML fragment.
    Toml(&'s str),
    /// The manifest is a valid TOML fragment (owned).
    // TODO: Change to Cow<'s, str>.
    TomlOwned(String),
}

impl<'s> Manifest<'s> {
    pub fn into_toml(self) -> MainResult<toml::value::Table> {
        use self::Manifest::*;
        match self {
            Toml(s) => toml::from_str(s),
            TomlOwned(ref s) => toml::from_str(s),
        }
        .map_err(|e| {
            MainError::Tag(
                "could not parse embedded manifest".into(),
                Box::new(MainError::Other(Box::new(e))),
            )
        })
    }
}

/**
Locates a manifest embedded in Rust source.

Returns `Some((manifest, source))` if it finds a manifest, `None` otherwise.
*/
fn find_embedded_manifest(s: &str) -> Option<(Manifest, &str)> {
    find_code_block_manifest(s)
}

#[test]
fn test_find_embedded_manifest() {
    use self::Manifest::*;

    let fem = find_embedded_manifest;

    assert_eq!(fem("fn main() {}"), None);

    assert_eq!(
        fem("
fn main() {}
"),
        None
    );

    // Ensure removed prefix manifests don't work.
    assert_eq!(
        fem(r#"
---
fn main() {}
"#),
        None
    );

    assert_eq!(
        fem("[dependencies]
time = \"0.1.25\"
---
fn main() {}
"),
        None
    );

    assert_eq!(
        fem("[dependencies]
time = \"0.1.25\"

fn main() {}
"),
        None
    );

    // Make sure we aren't just grabbing the *last* line.
    assert_eq!(
        fem("[dependencies]
time = \"0.1.25\"

fn main() {
    println!(\"Hi!\");
}
"),
        None
    );

    assert_eq!(
        fem(r#"//! ```Cargo
//! [dependencies]
//! time = "0.1.25"
//! ```
fn main() {}
"#),
        Some((
            TomlOwned(
                r#"[dependencies]
time = "0.1.25"
"#
                .into()
            ),
            r#"//! ```Cargo
//! [dependencies]
//! time = "0.1.25"
//! ```
fn main() {}
"#
        ))
    );

    assert_eq!(
        fem(r#"/*!
[dependencies]
time = "0.1.25"
*/
fn main() {}
"#),
        None
    );

    assert_eq!(
        fem(r#"/*!
```Cargo
[dependencies]
time = "0.1.25"
```
*/
fn main() {}
"#),
        Some((
            TomlOwned(
                r#"[dependencies]
time = "0.1.25"
"#
                .into()
            ),
            r#"/*!
```Cargo
[dependencies]
time = "0.1.25"
```
*/
fn main() {}
"#
        ))
    );

    assert_eq!(
        fem(r#"/*!
 * [dependencies]
 * time = "0.1.25"
 */
fn main() {}
"#),
        None
    );

    assert_eq!(
        fem(r#"/*!
 * ```Cargo
 * [dependencies]
 * time = "0.1.25"
 * ```
 */
fn main() {}
"#),
        Some((
            TomlOwned(
                r#"[dependencies]
time = "0.1.25"
"#
                .into()
            ),
            r#"/*!
 * ```Cargo
 * [dependencies]
 * time = "0.1.25"
 * ```
 */
fn main() {}
"#
        ))
    );
}

/**
Locates a "code block manifest" in Rust source.
*/
fn find_code_block_manifest(s: &str) -> Option<(Manifest, &str)> {
    /*
    This has to happen in a few steps.

    First, we will look for and slice out a contiguous, inner doc comment which must be *the very first thing* in the file.  `#[doc(...)]` attributes *are not supported*.  Multiple single-line comments cannot have any blank lines between them.

    Then, we need to strip off the actual comment markers from the content.  Including indentation removal, and taking out the (optional) leading line markers for block comments.  *sigh*

    Then, we need to take the contents of this doc comment and feed it to a Markdown parser.  We are looking for *the first* fenced code block with a language token of `cargo`.  This is extracted and pasted back together into the manifest.
    */
    let start = match RE_CRATE_COMMENT.captures(s) {
        Some(cap) => match cap.get(1) {
            Some(m) => m.start(),
            None => return None,
        },
        None => return None,
    };

    let comment = match extract_comment(&s[start..]) {
        Ok(s) => s,
        Err(err) => {
            error!("error slicing comment: {}", err);
            return None;
        }
    };

    scrape_markdown_manifest(&comment).map(|m| (Manifest::TomlOwned(m), s))
}

/**
Extracts the first `Cargo` fenced code block from a chunk of Markdown.
*/
fn scrape_markdown_manifest(content: &str) -> Option<String> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

    // To match librustdoc/html/markdown.rs, opts.
    let exts = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;

    let md = Parser::new_ext(content, exts);

    let mut found = false;
    let mut output = None;

    for item in md {
        match item {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info)))
                if info.to_lowercase() == "cargo" && output.is_none() =>
            {
                found = true;
            }
            Event::Text(ref text) if found => {
                let s = output.get_or_insert(String::new());
                s.push_str(&*text);
            }
            Event::End(Tag::CodeBlock(_)) if found => {
                found = false;
            }
            _ => (),
        }
    }

    output
}

#[test]
fn test_scrape_markdown_manifest() {
    macro_rules! smm {
        ($c:expr) => {
            scrape_markdown_manifest($c)
        };
    }

    assert_eq!(
        smm!(
            r#"There is no manifest in this comment.
"#
        ),
        None
    );

    assert_eq!(
        smm!(
            r#"There is no manifest in this comment.

```
This is not a manifest.
```

```rust
println!("Nor is this.");
```

    Or this.
"#
        ),
        None
    );

    assert_eq!(
        smm!(
            r#"This is a manifest:

```cargo
dependencies = { time = "*" }
```
"#
        ),
        Some(
            r#"dependencies = { time = "*" }
"#
            .into()
        )
    );

    assert_eq!(
        smm!(
            r#"This is *not* a manifest:

```
He's lying, I'm *totally* a manifest!
```

This *is*:

```cargo
dependencies = { time = "*" }
```
"#
        ),
        Some(
            r#"dependencies = { time = "*" }
"#
            .into()
        )
    );

    assert_eq!(
        smm!(
            r#"This is a manifest:

```cargo
dependencies = { time = "*" }
```

So is this, but it doesn't count:

```cargo
dependencies = { explode = true }
```
"#
        ),
        Some(
            r#"dependencies = { time = "*" }
"#
            .into()
        )
    );
}

/**
Extracts the contents of a Rust doc comment.
*/
fn extract_comment(s: &str) -> MainResult<String> {
    use std::cmp::min;

    fn n_leading_spaces(s: &str, n: usize) -> MainResult<()> {
        if !s.chars().take(n).all(|c| c == ' ') {
            return Err(format!("leading {:?} chars aren't all spaces: {:?}", n, s).into());
        }
        Ok(())
    }

    fn extract_block(s: &str) -> MainResult<String> {
        /*
        On every line:

        - update nesting level and detect end-of-comment
        - if margin is None:
            - if there appears to be a margin, set margin.
        - strip off margin marker
        - update the leading space counter
        - strip leading space
        - append content
        */
        let mut r = String::new();

        let margin_re = &*RE_MARGIN;
        let space_re = &*RE_SPACE;
        let nesting_re = &*RE_NESTING;

        let mut leading_space = None;
        let mut margin = None;
        let mut depth: u32 = 1;

        for line in s.lines() {
            if depth == 0 {
                break;
            }

            // Update nesting and look for end-of-comment.
            let mut end_of_comment = None;

            for (end, marker) in nesting_re.find_iter(line).map(|m| (m.start(), m.as_str())) {
                match (marker, depth) {
                    ("/*", _) => depth += 1,
                    ("*/", 1) => {
                        end_of_comment = Some(end);
                        depth = 0;
                        break;
                    }
                    ("*/", _) => depth -= 1,
                    _ => panic!("got a comment marker other than /* or */"),
                }
            }

            let line = end_of_comment.map(|end| &line[..end]).unwrap_or(line);

            // Detect and strip margin.
            margin = margin.or_else(|| margin_re.find(line).map(|m| m.as_str()));

            let line = if let Some(margin) = margin {
                let end = line
                    .char_indices()
                    .take(margin.len())
                    .map(|(i, c)| i + c.len_utf8())
                    .last()
                    .unwrap_or(0);
                &line[end..]
            } else {
                line
            };

            // Detect and strip leading indentation.
            leading_space = leading_space.or_else(|| space_re.find(line).map(|m| m.end()));

            /*
            Make sure we have only leading spaces.

            If we see a tab, fall over.  I *would* expand them, but that gets into the question of how *many* spaces to expand them to, and *where* is the tab, because tabs are tab stops and not just N spaces.

            Eurgh.
            */
            n_leading_spaces(line, leading_space.unwrap_or(0))?;

            let strip_len = min(leading_space.unwrap_or(0), line.len());
            let line = &line[strip_len..];

            // Done.
            r.push_str(line);

            // `lines` removes newlines.  Ideally, it wouldn't do that, but hopefully this shouldn't cause any *real* problems.
            r.push('\n');
        }

        Ok(r)
    }

    fn extract_line(s: &str) -> MainResult<String> {
        let mut r = String::new();

        let comment_re = &*RE_COMMENT;
        let space_re = &*RE_SPACE;

        let mut leading_space = None;

        for line in s.lines() {
            // Strip leading comment marker.
            let content = match comment_re.find(line) {
                Some(m) => &line[m.end()..],
                None => break,
            };

            // Detect and strip leading indentation.
            leading_space = leading_space.or_else(|| {
                space_re
                    .captures(content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.end())
            });

            /*
            Make sure we have only leading spaces.

            If we see a tab, fall over.  I *would* expand them, but that gets into the question of how *many* spaces to expand them to, and *where* is the tab, because tabs are tab stops and not just N spaces.

            Eurgh.
            */
            n_leading_spaces(content, leading_space.unwrap_or(0))?;

            let strip_len = min(leading_space.unwrap_or(0), content.len());
            let content = &content[strip_len..];

            // Done.
            r.push_str(content);

            // `lines` removes newlines.  Ideally, it wouldn't do that, but hopefully this shouldn't cause any *real* problems.
            r.push('\n');
        }

        Ok(r)
    }

    if let Some(stripped) = s.strip_prefix("/*!") {
        extract_block(stripped)
    } else if s.starts_with("//!") || s.starts_with("///") {
        extract_line(s)
    } else {
        Err("no doc comment found".into())
    }
}

#[test]
fn test_extract_comment() {
    macro_rules! ec {
        ($s:expr) => {
            extract_comment($s).map_err(|e| e.to_string())
        };
    }

    assert_eq!(ec!(r#"fn main () {}"#), Err("no doc comment found".into()));

    assert_eq!(
        ec!(r#"/*!
Here is a manifest:

```cargo
[dependencies]
time = "*"
```
*/
fn main() {}
"#),
        Ok(r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#
        .into())
    );

    assert_eq!(
        ec!(r#"/*!
 * Here is a manifest:
 *
 * ```cargo
 * [dependencies]
 * time = "*"
 * ```
 */
fn main() {}
"#),
        Ok(r#"
Here is a manifest:

```cargo
[dependencies]
time = "*"
```

"#
        .into())
    );

    assert_eq!(
        ec!(r#"//! Here is a manifest:
//!
//! ```cargo
//! [dependencies]
//! time = "*"
//! ```
fn main() {}
"#),
        Ok(r#"Here is a manifest:

```cargo
[dependencies]
time = "*"
```
"#
        .into())
    );
}

/**
Generates a default Cargo manifest for the given input.
*/
fn default_manifest(input: &Input, input_id: &OsString) -> MainResult<toml::value::Table> {
    let mani_str = {
        let pkg_name = input.package_name();
        let bin_name = format!("{}_{}", &*pkg_name, input_id.to_str().unwrap());
        let mut subs = HashMap::with_capacity(3);
        subs.insert(consts::MANI_NAME_SUB, &*pkg_name);
        subs.insert(consts::MANI_BIN_NAME_SUB, &*bin_name);
        subs.insert(consts::MANI_FILE_SUB, input.safe_name());
        templates::expand(consts::DEFAULT_MANIFEST, &subs)?
    };
    toml::from_str(&mani_str).map_err(|e| {
        MainError::Tag(
            "could not parse default manifest".into(),
            Box::new(MainError::Other(Box::new(e))),
        )
    })
}

/**
Given two Cargo manifests, merges the second *into* the first.

Note that the "merge" in this case is relatively simple: only *top-level* tables are actually merged; everything else is just outright replaced.
*/
fn merge_manifest(
    mut into_t: toml::value::Table,
    from_t: toml::value::Table,
) -> MainResult<toml::value::Table> {
    for (k, v) in from_t {
        match v {
            toml::Value::Table(from_t) => {
                // Merge.
                match into_t.entry(k) {
                    toml::map::Entry::Vacant(e) => {
                        e.insert(toml::Value::Table(from_t));
                    }
                    toml::map::Entry::Occupied(e) => {
                        let into_t = as_table_mut(e.into_mut()).ok_or(
                            "cannot merge manifests: cannot merge \
                                table and non-table values",
                        )?;
                        into_t.extend(from_t);
                    }
                }
            }
            v => {
                // Just replace.
                into_t.insert(k, v);
            }
        }
    }

    return Ok(into_t);

    fn as_table_mut(t: &mut toml::Value) -> Option<&mut toml::value::Table> {
        match *t {
            toml::Value::Table(ref mut t) => Some(t),
            _ => None,
        }
    }
}

/**
Given a Cargo manifest, attempts to rewrite relative file paths to absolute ones, allowing the manifest to be relocated.
*/
fn fix_manifest_paths(mani: toml::value::Table, base: &Path) -> MainResult<toml::value::Table> {
    // Values that need to be rewritten:
    let paths: &[&[&str]] = &[
        &["build-dependencies", "*", "path"],
        &["dependencies", "*", "path"],
        &["dev-dependencies", "*", "path"],
        &["package", "build"],
        &["target", "*", "dependencies", "*", "path"],
    ];

    let mut mani = toml::Value::Table(mani);

    for path in paths {
        iterate_toml_mut_path(&mut mani, path, &mut |v| {
            if let toml::Value::String(ref mut s) = *v {
                if Path::new(s).is_relative() {
                    let p = base.join(&*s);
                    if let Some(p) = p.to_str() {
                        *s = p.into()
                    }
                }
            }
            Ok(())
        })?
    }

    match mani {
        toml::Value::Table(mani) => Ok(mani),
        _ => unreachable!(),
    }
}

/**
Iterates over the specified TOML values via a path specification.
*/
fn iterate_toml_mut_path<F>(
    base: &mut toml::Value,
    path: &[&str],
    on_each: &mut F,
) -> MainResult<()>
where
    F: FnMut(&mut toml::Value) -> MainResult<()>,
{
    if path.is_empty() {
        return on_each(base);
    }

    let cur = path[0];
    let tail = &path[1..];

    if cur == "*" {
        if let toml::Value::Table(ref mut tab) = *base {
            for (_, v) in tab {
                iterate_toml_mut_path(v, tail, on_each)?;
            }
        }
    } else if let toml::Value::Table(ref mut tab) = *base {
        if let Some(v) = tab.get_mut(cur) {
            iterate_toml_mut_path(v, tail, on_each)?;
        }
    }

    Ok(())
}
