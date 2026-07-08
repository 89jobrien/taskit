use std::{fs, path::Path};
use taskit_types::error::{TaskitError, TaskitResultExt};

/// Count lines in `content` containing `pattern`, returning `(line_number, line)` pairs.
fn match_lines<'a>(content: &'a str, pattern: &str) -> Vec<(usize, &'a str)> {
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(pattern))
        .collect()
}

pub fn run(
    file: &Path,
    pattern: &str,
    expected: usize,
    warn_only: bool,
) -> Result<(), TaskitError> {
    let content = fs::read_to_string(file)
        .err_context_with(|| format!("failed to read {}", file.display()))?;

    let matches = match_lines(&content, pattern);
    let count = matches.len();
    taskit_output::taskit_progress!(
        "check-protocol-sites: found {count} `{pattern}` construction site(s) \
         in {} (expected {expected})",
        file.display()
    );

    if count != expected {
        taskit_output::taskit_err!(
            "WARN: construction site count changed: expected {expected}, got {count}."
        );
        for (lineno, line) in &matches {
            taskit_output::taskit_err!("{}:{}: {}", file.display(), lineno + 1, line.trim());
        }
        if warn_only {
            return Ok(());
        }
        return Err(TaskitError::other(format!(
            "construction site count mismatch ({count} != {expected})"
        )));
    }

    taskit_output::taskit_ok!("OK: construction site count matches.");
    for (lineno, line) in &matches {
        taskit_output::taskit_progress!("{}:{}: {}", file.display(), lineno + 1, line.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_file(content: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("taskit_psites_{pid}_{n}.rs"));
        std::fs::write(&path, content).expect("write tmp file");
        path
    }

    // --- match_lines unit tests ---

    #[test]
    fn match_lines_returns_empty_when_no_match() {
        let result = match_lines("fn foo() {}\nlet x = 1;\n", "CreateSessionRequest {");
        assert!(result.is_empty());
    }

    #[test]
    fn match_lines_returns_correct_count() {
        let content = "CreateSessionRequest {\nfoo\nCreateSessionRequest {\n";
        let result = match_lines(content, "CreateSessionRequest {");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn match_lines_reports_correct_line_numbers() {
        let content = "line0\nCreateSessionRequest {\nline2\n";
        let result = match_lines(content, "CreateSessionRequest {");
        assert_eq!(result[0].0, 1); // zero-indexed line 1
    }

    #[test]
    fn match_lines_pattern_substring_matches() {
        let result = match_lines(
            "    let r = CreateSessionRequest { foo: 1 };",
            "CreateSessionRequest {",
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn match_lines_does_not_match_partial_pattern() {
        let result = match_lines("CreateSessionReq {\n", "CreateSessionRequest {");
        assert!(result.is_empty());
    }

    // --- run() integration tests via tempfile ---

    #[test]
    fn run_passes_when_count_matches() {
        let p = tmp_file(
            "CreateSessionRequest {\nCreateSessionRequest {\nCreateSessionRequest {\nCreateSessionRequest {\n",
        );
        assert!(run(&p, "CreateSessionRequest {", 4, false).is_ok());
    }

    #[test]
    fn run_fails_when_count_too_low() {
        let p = tmp_file("CreateSessionRequest {\n");
        let err = run(&p, "CreateSessionRequest {", 4, false).unwrap_err();
        assert!(err.to_string().contains("mismatch"));
    }

    #[test]
    fn run_fails_when_count_too_high() {
        let p = tmp_file(
            "CreateSessionRequest {\nCreateSessionRequest {\nCreateSessionRequest {\nCreateSessionRequest {\nCreateSessionRequest {\n",
        );
        assert!(run(&p, "CreateSessionRequest {", 4, false).is_err());
    }

    #[test]
    fn run_warn_only_returns_ok_on_mismatch() {
        let p = tmp_file("CreateSessionRequest {\n");
        assert!(run(&p, "CreateSessionRequest {", 4, true).is_ok());
    }

    #[test]
    fn run_zero_expected_passes_on_empty_file() {
        let p = tmp_file("fn foo() {}\n");
        assert!(run(&p, "CreateSessionRequest {", 0, false).is_ok());
    }

    #[test]
    fn run_returns_error_for_missing_file() {
        let p = std::path::Path::new("/tmp/__taskit_nonexistent_file_xyz__.rs");
        assert!(run(p, "pattern", 0, false).is_err());
    }
}
