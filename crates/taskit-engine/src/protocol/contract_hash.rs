use sha2::{Digest, Sha256};

/// Normalize a Rust contract source file for stable hashing.
///
/// Strips blank lines, line comments, doc comments, trailing inline comments,
/// and `#[cfg(test)] mod tests { ... }` blocks.
pub fn normalize(content: &str) -> String {
    let mut normalized = Vec::new();
    let mut skip_cfg_test = false;
    let mut skipping_test_module = false;
    let mut test_module_depth = 0isize;

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();

        if skipping_test_module {
            test_module_depth += brace_delta(trimmed);
            if test_module_depth <= 0 {
                skipping_test_module = false;
                test_module_depth = 0;
            }
            continue;
        }

        if trimmed == "#[cfg(test)]" {
            skip_cfg_test = true;
            continue;
        }

        if skip_cfg_test && trimmed.starts_with("mod tests") {
            test_module_depth = brace_delta(trimmed);
            if test_module_depth > 0 {
                skipping_test_module = true;
            }
            skip_cfg_test = false;
            continue;
        }

        // Don't consume skip_cfg_test on comment/blank lines — a comment between
        // `#[cfg(test)]` and `mod tests {` should not prevent the module from being stripped.
        if skip_cfg_test && !trimmed.starts_with("//") && !trimmed.is_empty() {
            skip_cfg_test = false;
        }

        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let line = strip_trailing_comment(trimmed).trim();
        if !line.is_empty() {
            normalized.push(line.to_string());
        }
    }

    normalized.join("\n") + "\n"
}

pub fn hash(normalized: &str) -> String {
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)
}

// NOTE: This split is intentionally simple. It will incorrectly strip content
// after " //" inside string literals (e.g. `let s = "http://x.com/path";`).
// For the small set of tracked contract files this is not a practical concern.
// If a tracked file gains string literals containing " //", prefer adding a
// normalisation exclusion rather than implementing full quote-aware parsing.
fn strip_trailing_comment(line: &str) -> &str {
    line.split_once(" //").map_or(line, |(before, _)| before)
}

fn brace_delta(line: &str) -> isize {
    line.chars().fold(0, |d, ch| match ch {
        '{' => d + 1,
        '}' => d - 1,
        _ => d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_comments_and_test_modules() {
        let first = normalize(
            r#"
            //! docs
            #[derive(Debug)]
            pub enum Wire { Request }

            #[cfg(test)]
            mod tests {
                #[test]
                fn ignored() { assert_eq!(1, 1); }
            }
            "#,
        );
        let second = normalize(
            r#"
            // different docs
            #[derive(Debug)]
            pub enum Wire { Request }

            #[cfg(test)]
            mod tests {
                #[test]
                fn also_ignored() { assert_eq!(2, 2); }
            }
            "#,
        );
        assert_eq!(hash(&first), hash(&second));
    }

    #[test]
    fn hash_changes_when_contract_changes() {
        let before = normalize("pub enum Wire { Request }\n");
        let after = normalize("pub enum Wire { Request, Response }\n");
        assert_ne!(hash(&before), hash(&after));
    }

    #[test]
    fn strip_trailing_comment_removes_inline_comment() {
        assert_eq!(
            strip_trailing_comment("let x = 1; // comment"),
            "let x = 1;"
        );
    }

    #[test]
    fn strip_trailing_comment_no_comment_returns_original() {
        assert_eq!(strip_trailing_comment("let x = 1;"), "let x = 1;");
    }

    #[test]
    fn strip_trailing_comment_false_positive_on_space_slash_slash_in_string() {
        // Known limitation: " //" inside a string literal is incorrectly stripped.
        // e.g. `let s = "a // b";` loses " b" after the " //".
        // URLs like "http://x.com" are safe because "://" has no leading space.
        // Acceptable for the small set of tracked contract files.
        assert_eq!(
            strip_trailing_comment(r#"let s = "a // b";"#),
            r#"let s = "a"#
        );
    }

    #[test]
    fn strip_trailing_comment_line_comment_at_start_is_unchanged() {
        // Lines starting with `//` are filtered earlier; this fn only strips trailing comments.
        // A leading `//` has no space before it so the split_once(" //") won't match.
        assert_eq!(strip_trailing_comment("// pure comment"), "// pure comment");
    }

    #[test]
    fn brace_delta_open_braces() {
        assert_eq!(brace_delta("fn foo() {"), 1);
        assert_eq!(brace_delta("fn foo() { let _ = {"), 2);
    }

    #[test]
    fn brace_delta_close_braces() {
        assert_eq!(brace_delta("}"), -1);
        assert_eq!(brace_delta("} }"), -2);
    }

    #[test]
    fn brace_delta_balanced() {
        assert_eq!(brace_delta("{ }"), 0);
    }

    #[test]
    fn brace_delta_empty() {
        assert_eq!(brace_delta(""), 0);
    }

    #[test]
    fn normalize_cfg_test_not_followed_by_mod_tests_is_not_skipped() {
        // #[cfg(test)] on something other than `mod tests` should not strip the next line.
        let input = "#[cfg(test)]\nstruct NotATestMod;\n";
        let result = normalize(input);
        assert!(
            result.contains("struct NotATestMod;"),
            "non-test-mod cfg should not be stripped"
        );
    }

    #[test]
    fn normalize_nested_braces_inside_test_module_handled() {
        let input = r#"
pub struct Foo;

#[cfg(test)]
mod tests {
    fn inner() {
        let _ = { { } };
    }
}
"#;
        let result = normalize(input);
        assert!(result.contains("pub struct Foo;"));
        assert!(
            !result.contains("inner"),
            "test module contents should be stripped"
        );
    }

    #[test]
    fn normalize_cfg_test_with_comment_between_and_mod_tests_is_stripped() {
        // A comment line between `#[cfg(test)]` and `mod tests {` must not prevent
        // the test module from being stripped.
        let input = "#[cfg(test)]\n// a comment\nmod tests {\n    fn t() {}\n}\n";
        let result = normalize(input);
        assert!(
            !result.contains("mod tests"),
            "test module should be stripped even with comment between cfg and mod"
        );
    }

    #[test]
    fn normalize_whitespace_only_file_produces_empty_output() {
        assert_eq!(normalize("   \n\n\t\n"), "\n");
    }

    #[test]
    fn normalize_is_idempotent() {
        let input = r#"
// comment
pub struct Foo {
    pub bar: u32, // inline
}
"#;
        let first = normalize(input);
        let second = normalize(&first);
        assert_eq!(first, second);
    }
}
