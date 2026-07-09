use taskit_types::error::TaskitError;
use taskit_types::step::{DiagnosticLevel, DiagnosticRecord};
use xshell::cmd;

use crate::{ctx::Ctx, util};

pub fn run(
    ctx: &Ctx,
    crate_name: Option<&str>,
    use_affected: bool,
    continue_on_error: bool,
    offline: bool,
) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    let mut extra: Vec<String> = vec![
        "--status-level".into(),
        "none".into(),
        "--final-status-level".into(),
        "fail".into(),
        "--hide-progress-bar".into(),
    ];
    if continue_on_error {
        extra.push("--no-fail-fast".into());
    } else {
        extra.push("--fail-fast".into());
    }
    if offline && let Some(expr) = ws.offline_skip_expr() {
        extra.extend(["-E".into(), expr]);
    }
    let extra = extra.as_slice();
    util::run_per_crate(
        sh,
        ws,
        crate_name,
        use_affected,
        continue_on_error,
        |sh, name| {
            ctx.run(cmd!(
                sh,
                "cargo nextest run --locked -p {name} --all-targets {extra...}"
            ))
        },
        |sh| {
            ctx.run(cmd!(
                sh,
                "cargo nextest run --locked --workspace --all-targets {extra...}"
            ))
        },
    )
}

/// Run nextest with `--message-format libtest-json` and parse diagnostics.
///
/// Returns `(success, diagnostics)` where each failed test becomes a
/// `DiagnosticRecord`.
pub fn run_capturing(
    ctx: &Ctx,
    offline: bool,
) -> Result<(bool, Vec<DiagnosticRecord>), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    let mut extra: Vec<String> = vec![
        "--status-level".into(),
        "all".into(),
        "--hide-progress-bar".into(),
    ];
    if offline && let Some(expr) = ws.offline_skip_expr() {
        extra.extend(["-E".into(), expr]);
    }
    let extra_slice = extra.as_slice();
    let captured = ctx.run_capture(cmd!(
        sh,
        "cargo nextest run --locked --workspace --all-targets --message-format libtest-json {extra_slice...}"
    ))?;

    let diagnostics = parse_nextest_json(&captured.stdout);
    Ok((captured.success, diagnostics))
}

/// Parse nextest libtest-json output into `DiagnosticRecord`s for failures.
pub fn parse_nextest_json(json_lines: &str) -> Vec<DiagnosticRecord> {
    let mut records = Vec::new();
    for line in json_lines.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // libtest-json events: {"type":"test","event":"failed","name":"...","stdout":"..."}
        let event_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let event = msg.get("event").and_then(|e| e.as_str()).unwrap_or("");

        if event_type != "test" {
            continue;
        }
        if event != "failed" {
            continue;
        }

        let name = msg
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string();
        let stdout = msg.get("stdout").and_then(|s| s.as_str()).unwrap_or("");

        // Try to extract file:line from panic message in stdout
        // Pattern: "thread '...' panicked at path/file.rs:LINE:COL"
        let (file, line_num) = parse_panic_location(stdout);

        let rule_id = if stdout.contains("panicked") {
            "TE002".to_string() // TestError (panic)
        } else {
            "TE001".to_string() // TestFailure
        };

        let message = if stdout.is_empty() {
            name.clone()
        } else {
            // Take first meaningful line from stdout
            let first_line = stdout
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or(&name);
            format!("{name} -- {first_line}")
        };

        records.push(DiagnosticRecord {
            rule_id,
            message,
            level: DiagnosticLevel::Error,
            file,
            line: line_num,
            column: None,
        });
    }
    records
}

/// Extract file path and line number from a panic message.
fn parse_panic_location(stdout: &str) -> (Option<String>, Option<usize>) {
    // Look for "panicked at path/to/file.rs:LINE:COL"
    for line in stdout.lines() {
        if let Some(idx) = line.find("panicked at ") {
            let after = &line[idx + "panicked at ".len()..];
            // Format: "file.rs:LINE:COL" or "'message', file.rs:LINE:COL"
            let loc = if let Some(comma_idx) = after.rfind(", ") {
                &after[comma_idx + 2..]
            } else {
                after
            };
            let parts: Vec<&str> = loc.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let file = parts[0].trim().to_string();
                let line_num = parts[1].trim().parse::<usize>().ok();
                if line_num.is_some() {
                    return (Some(file), line_num);
                }
            }
        }
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkspaceConfig;

    #[test]
    fn offline_skip_expr_returns_none_by_default() {
        let ws = WorkspaceConfig::default();
        assert!(ws.offline_skip_expr().is_none());
    }

    #[test]
    fn offline_skip_expr_returns_configured_value() {
        let ws = WorkspaceConfig {
            offline_skip: Some("not test(slow)".into()),
            ..Default::default()
        };
        assert_eq!(ws.offline_skip_expr().as_deref(), Some("not test(slow)"));
    }

    #[test]
    fn parse_nextest_json_empty() {
        assert!(parse_nextest_json("").is_empty());
    }

    #[test]
    fn parse_nextest_json_passing_test_ignored() {
        let input = r#"{"type":"test","event":"ok","name":"tests::it_works"}"#;
        assert!(parse_nextest_json(input).is_empty());
    }

    #[test]
    fn parse_nextest_json_failed_test() {
        let input = r#"{"type":"test","event":"failed","name":"tests::bad","stdout":"thread 'tests::bad' panicked at src/lib.rs:42:5\nassertion failed"}"#;
        let records = parse_nextest_json(input);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "TE002");
        assert_eq!(records[0].level, DiagnosticLevel::Error);
        assert_eq!(records[0].file.as_deref(), Some("src/lib.rs"));
        assert_eq!(records[0].line, Some(42));
    }

    #[test]
    fn parse_nextest_json_failed_without_panic() {
        let input =
            r#"{"type":"test","event":"failed","name":"tests::bad","stdout":"assertion failed"}"#;
        let records = parse_nextest_json(input);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "TE001");
    }

    #[test]
    fn parse_panic_location_extracts_file_line() {
        let stdout = "thread 'test' panicked at src/main.rs:10:5\nnote: run with";
        let (file, line) = parse_panic_location(stdout);
        assert_eq!(file.as_deref(), Some("src/main.rs"));
        assert_eq!(line, Some(10));
    }

    #[test]
    fn parse_panic_location_with_message() {
        let stdout = "thread 'test' panicked at 'assertion failed', src/lib.rs:42:1";
        let (file, line) = parse_panic_location(stdout);
        assert_eq!(file.as_deref(), Some("src/lib.rs"));
        assert_eq!(line, Some(42));
    }

    #[test]
    fn parse_panic_location_no_match() {
        let (file, line) = parse_panic_location("just some output");
        assert!(file.is_none());
        assert!(line.is_none());
    }

    #[test]
    fn parse_nextest_json_suite_events_ignored() {
        let input = r#"{"type":"suite","event":"started","test_count":5}"#;
        assert!(parse_nextest_json(input).is_empty());
    }
}
