use std::borrow::Cow;
use std::collections::HashMap;

use regex::Regex;

static RE_SUB: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r#"#\{([A-Za-z_][A-Za-z0-9_]*)}"#).unwrap());

pub fn expand(src: &str, subs: &HashMap<&str, &str>) -> anyhow::Result<String> {
    // The estimate of final size is the sum of the size of all the input.
    let sub_size = subs.iter().map(|(_, v)| v.len()).sum::<usize>();
    let est_size = src.len() + sub_size;

    let mut anchor = 0;
    let mut result = String::with_capacity(est_size);

    for m in RE_SUB.captures_iter(src) {
        // Concatenate the static bit just before the match.
        let (m_start, m_end) = {
            let m_0 = m.get(0).unwrap();
            (m_0.start(), m_0.end())
        };
        let prior_slice = anchor..m_start;
        anchor = m_end;
        result.push_str(&src[prior_slice]);

        // Concat the substitution.
        let sub_name = m.get(1).unwrap().as_str();
        match subs.get(sub_name) {
            Some(s) => result.push_str(s),
            None => {
                anyhow::bail!("substitution `{sub_name}` in template is unknown")
            }
        }
    }
    result.push_str(&src[anchor..]);
    Ok(result)
}

/// Attempts to locate and load the contents of the specified template.
pub fn get_template(name: &str) -> anyhow::Result<Cow<'static, str>> {
    if let Some(text) = builtin_template(name) {
        return Ok(text.into());
    }
    anyhow::bail!("No such template: {name}");
}

fn builtin_template(name: &str) -> Option<&'static str> {
    Some(match name {
        "expr" => EXPR_TEMPLATE,
        "file" => FILE_TEMPLATE,
        _ => return None,
    })
}

/// The template used for script file inputs.
pub const FILE_TEMPLATE: &str = r#"#{script}"#;

/// The template used for `--expr` input.
pub const EXPR_TEMPLATE: &str = r#"
use std::any::{Any, TypeId};

fn main() {
    let exit_code = match try_main() {
        Ok(()) => None,
        Err(e) => {
            use std::io::{self, Write};
            let _ = writeln!(io::stderr(), "Error: {}", e);
            Some(1)
        },
    };
    if let Some(exit_code) = exit_code {
        std::process::exit(exit_code);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    fn _rust_script_is_empty_tuple<T: ?Sized + Any>(_s: &T) -> bool {
        TypeId::of::<()>() == TypeId::of::<T>()
    }
    match {#{script}} {
        __rust_script_expr if !_rust_script_is_empty_tuple(&__rust_script_expr) => println!("{:?}", __rust_script_expr),
        _ => {}
    }
    Ok(())
}
"#;
