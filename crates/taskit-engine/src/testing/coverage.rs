use anyhow::Context;
use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::is_dry_run;

pub fn run(sh: &Shell, pkg: &str, threshold: f64) -> Result<(), TaskitError> {
    eprintln!("Running coverage for {pkg} (threshold: {threshold}%)...");
    if is_dry_run() {
        eprintln!("dry-run: cargo llvm-cov --locked -p {pkg} --lib --json");
        return Ok(());
    }
    let json = cmd!(sh, "cargo llvm-cov --locked -p {pkg} --lib --json")
        .read()
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;

    let pct = parse_line_coverage(&json).context("failed to parse cargo llvm-cov --json output")?;

    eprintln!("Coverage: {pct:.1}%");
    if pct < threshold {
        return Err(anyhow::anyhow!("Coverage {pct:.1}% is below threshold {threshold}%").into());
    }
    eprintln!("Coverage {pct:.1}% >= {threshold}% threshold — OK");
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
