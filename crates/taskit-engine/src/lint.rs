use taskit_types::error::TaskitError;
use taskit_types::step::{DiagnosticLevel, DiagnosticRecord};
use xshell::cmd;

use crate::{ctx::Ctx, util};

pub fn run(
    ctx: &Ctx,
    crate_name: Option<&str>,
    use_affected: bool,
    continue_on_error: bool,
) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    util::run_per_crate(
        sh,
        ws,
        crate_name,
        use_affected,
        continue_on_error,
        |sh, name| {
            ctx.run(cmd!(
                sh,
                "cargo clippy --locked --quiet -p {name} --all-targets -- -D warnings"
            ))
        },
        |sh| {
            ctx.run(cmd!(
                sh,
                "cargo clippy --locked --quiet --all-targets --workspace -- -D warnings"
            ))
        },
    )
}

/// Run clippy with `--message-format=json` and parse diagnostics.
///
/// Returns `(success, diagnostics)`. The bool indicates whether clippy
/// exited cleanly (no warnings treated as errors).
pub fn run_capturing(ctx: &Ctx) -> Result<(bool, Vec<DiagnosticRecord>), TaskitError> {
    let sh = &ctx.sh;

    let captured = ctx.run_capture(cmd!(
        sh,
        "cargo clippy --locked --quiet --all-targets --workspace --message-format=json -- -D warnings"
    ))?;

    let diagnostics = parse_clippy_json(&captured.stdout);
    // If affected-crate scoping is configured, filter could happen here.
    // For now we capture workspace-wide.
    Ok((captured.success, diagnostics))
}

/// Parse clippy's JSON message stream into `DiagnosticRecord`s.
pub fn parse_clippy_json(json_lines: &str) -> Vec<DiagnosticRecord> {
    let mut records = Vec::new();
    for line in json_lines.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // clippy emits {"reason":"compiler-message","message":{...}}
        if msg.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(message) = msg.get("message") else {
            continue;
        };
        let level_str = message.get("level").and_then(|l| l.as_str()).unwrap_or("");
        let level = match level_str {
            "error" => DiagnosticLevel::Error,
            "warning" => DiagnosticLevel::Warning,
            _ => DiagnosticLevel::Note,
        };
        // Skip "aborting due to N previous errors" summary messages
        if level_str == "error"
            && message
                .get("message")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.starts_with("aborting due to"))
        {
            continue;
        }
        let text = message
            .get("rendered")
            .or_else(|| message.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let code = message
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract primary span location
        let spans = message.get("spans").and_then(|s| s.as_array());
        let primary = spans.and_then(|arr| {
            arr.iter()
                .find(|s| s.get("is_primary").and_then(|p| p.as_bool()) == Some(true))
                .or_else(|| arr.first())
        });
        let file = primary
            .and_then(|s| s.get("file_name"))
            .and_then(|f| f.as_str())
            .map(String::from);
        let line = primary
            .and_then(|s| s.get("line_start"))
            .and_then(|l| l.as_u64())
            .map(|l| l as usize);
        let column = primary
            .and_then(|s| s.get("column_start"))
            .and_then(|c| c.as_u64())
            .map(|c| c as usize);

        records.push(DiagnosticRecord {
            rule_id: code,
            message: text,
            level,
            file,
            line,
            column,
        });
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clippy_json_empty_input() {
        assert!(parse_clippy_json("").is_empty());
    }

    #[test]
    fn parse_clippy_json_non_message_lines_skipped() {
        let input = r#"{"reason":"compiler-artifact","target":{"name":"foo"}}"#;
        assert!(parse_clippy_json(input).is_empty());
    }

    #[test]
    fn parse_clippy_json_warning() {
        let input = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable","code":{"code":"unused_variables"},"spans":[{"file_name":"src/main.rs","line_start":10,"column_start":5,"is_primary":true}],"rendered":"warning: unused variable `x`"}}"#;
        let records = parse_clippy_json(input);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "unused_variables");
        assert_eq!(records[0].level, DiagnosticLevel::Warning);
        assert_eq!(records[0].file.as_deref(), Some("src/main.rs"));
        assert_eq!(records[0].line, Some(10));
        assert_eq!(records[0].column, Some(5));
    }

    #[test]
    fn parse_clippy_json_skips_abort_message() {
        let input = r#"{"reason":"compiler-message","message":{"level":"error","message":"aborting due to 3 previous errors","code":null,"spans":[],"rendered":"error: aborting due to 3 previous errors"}}"#;
        assert!(parse_clippy_json(input).is_empty());
    }

    #[test]
    fn parse_clippy_json_multiple_lines() {
        let w1 = r#"{"reason":"compiler-message","message":{"level":"warning","message":"a","code":{"code":"w1"},"spans":[],"rendered":"a"}}"#;
        let w2 = r#"{"reason":"compiler-message","message":{"level":"warning","message":"b","code":{"code":"w2"},"spans":[],"rendered":"b"}}"#;
        let input = format!("{w1}\n{w2}");
        assert_eq!(parse_clippy_json(&input).len(), 2);
    }
}
