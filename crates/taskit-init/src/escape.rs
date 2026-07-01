//! Escaping for values interpolated into generated files.
//!
//! Discovered crate directories, package names, and surface paths come from
//! the filesystem and can contain quotes, backslashes, or newlines. Rendering
//! them verbatim would let a hostile directory name inject arbitrary TOML or
//! YAML into the files `taskit init` writes.

/// Render `s` as a quoted TOML basic string, escaping as needed.
pub fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    push_escaped(&mut out, s);
    out.push('"');
    out
}

/// Render `s` as a double-quoted YAML scalar, escaping as needed.
///
/// TOML basic strings and YAML double-quoted scalars share the escape set we
/// need (`\\`, `\"`, `\n`, `\t`, `\r`, `\u00XX`).
pub fn yaml_string(s: &str) -> String {
    toml_string(s)
}

/// Strip characters that would break out of a single-line `#` comment.
pub fn comment_text(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_strings_are_just_quoted() {
        assert_eq!(toml_string("my-lib"), "\"my-lib\"");
        assert_eq!(yaml_string("fmt --check"), "\"fmt --check\"");
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        assert_eq!(toml_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(toml_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn newlines_cannot_break_out() {
        let hostile = "x\"]\n[ci]\nsteps = []";
        let escaped = toml_string(hostile);
        assert!(!escaped.contains('\n'));
        // Round-trip through the TOML parser to prove containment.
        let doc = format!("key = {escaped}\n");
        let parsed: toml::Value = toml::from_str(&doc).unwrap();
        assert_eq!(parsed["key"].as_str(), Some(hostile));
    }

    #[test]
    fn control_chars_are_escaped() {
        assert_eq!(toml_string("a\u{1}b"), "\"a\\u0001b\"");
    }

    #[test]
    fn comment_text_strips_newlines() {
        assert_eq!(comment_text("a\nb\rc"), "abc");
        assert_eq!(comment_text("plain"), "plain");
    }
}
