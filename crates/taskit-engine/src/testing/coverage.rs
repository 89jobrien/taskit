use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, pkg: &str, threshold: f64) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    taskit_output::taskit_progress!("Running coverage for {pkg} (threshold: {threshold}%)...");
    if ctx.dry_run {
        taskit_output::taskit_dry!("cargo llvm-cov --locked -p {pkg} --lib --json");
        return Ok(());
    }
    let json = cmd!(sh, "cargo llvm-cov --locked -p {pkg} --lib --json")
        .read()
        .map_err(TaskitError::other)?;

    let pct = parse_line_coverage(&json)
        .ok_or_else(|| TaskitError::other("failed to parse cargo llvm-cov --json output"))?;

    taskit_output::taskit_progress!("Coverage: {pct:.1}%");
    if pct < threshold {
        return Err(TaskitError::other(format!(
            "Coverage {pct:.1}% is below threshold {threshold}%"
        )));
    }
    taskit_output::taskit_ok!("Coverage {pct:.1}% >= {threshold}% threshold — OK");
    Ok(())
}

fn parse_line_coverage(json: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v["data"][0]["totals"]["lines"]["percent"].as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_coverage_extracts_percent() {
        let json = serde_json::json!({
            "data": [{
                "totals": {
                    "lines": { "count": 100, "covered": 85, "percent": 85.0 }
                }
            }]
        })
        .to_string();
        assert_eq!(parse_line_coverage(&json), Some(85.0));
    }

    #[test]
    fn parse_line_coverage_returns_none_on_invalid_json() {
        assert_eq!(parse_line_coverage("not json"), None);
    }

    #[test]
    fn parse_line_coverage_returns_none_on_missing_field() {
        let json = serde_json::json!({ "data": [{}] }).to_string();
        assert_eq!(parse_line_coverage(&json), None);
    }

    #[test]
    fn parse_line_coverage_zero_percent() {
        let json = serde_json::json!({
            "data": [{ "totals": { "lines": { "percent": 0.0 } } }]
        })
        .to_string();
        assert_eq!(parse_line_coverage(&json), Some(0.0));
    }

    #[test]
    fn parse_line_coverage_hundred_percent() {
        let json = serde_json::json!({
            "data": [{ "totals": { "lines": { "percent": 100.0 } } }]
        })
        .to_string();
        assert_eq!(parse_line_coverage(&json), Some(100.0));
    }

    #[test]
    fn parse_line_coverage_empty_data_array_returns_none() {
        let json = serde_json::json!({ "data": [] }).to_string();
        assert_eq!(parse_line_coverage(&json), None);
    }

    #[test]
    fn parse_line_coverage_uses_first_data_entry() {
        // When multiple data entries are present, the first one should be used.
        let json = serde_json::json!({
            "data": [
                { "totals": { "lines": { "percent": 42.0 } } },
                { "totals": { "lines": { "percent": 99.0 } } }
            ]
        })
        .to_string();
        assert_eq!(parse_line_coverage(&json), Some(42.0));
    }

    #[test]
    fn parse_line_coverage_returns_none_when_percent_is_string() {
        let json = serde_json::json!({
            "data": [{ "totals": { "lines": { "percent": "high" } } }]
        })
        .to_string();
        assert_eq!(parse_line_coverage(&json), None);
    }

    #[test]
    fn parse_line_coverage_returns_none_when_data_is_not_array() {
        let json = serde_json::json!({ "data": {} }).to_string();
        assert_eq!(parse_line_coverage(&json), None);
    }
}
