use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// What to measure coverage over.
pub enum CoverageScope<'a> {
    Package(&'a str),
    Workspace,
}

impl CoverageScope<'_> {
    fn label(&self) -> String {
        match self {
            CoverageScope::Package(pkg) => (*pkg).to_string(),
            CoverageScope::Workspace => "workspace".to_string(),
        }
    }
}

/// Measure and gate coverage for a single package against `threshold`.
pub fn run(ctx: &Ctx, pkg: &str, threshold: f64) -> Result<(), TaskitError> {
    gate(ctx, &CoverageScope::Package(pkg), threshold)
}

/// Measure and gate coverage across the whole workspace against `threshold`.
pub fn run_workspace(ctx: &Ctx, threshold: f64) -> Result<(), TaskitError> {
    gate(ctx, &CoverageScope::Workspace, threshold)
}

fn gate(ctx: &Ctx, scope: &CoverageScope, threshold: f64) -> Result<(), TaskitError> {
    let label = scope.label();
    taskit_output::taskit_progress!("Running coverage for {label} (threshold: {threshold}%)...");
    let Some(pct) = collect_percent(ctx, scope)? else {
        return Ok(()); // dry-run
    };

    taskit_output::taskit_progress!("Coverage: {pct:.1}%");
    if pct < threshold {
        return Err(TaskitError::other(format!(
            "Coverage {pct:.1}% is below threshold {threshold}%"
        )));
    }
    taskit_output::taskit_ok!("Coverage {pct:.1}% >= {threshold}% threshold — OK");
    Ok(())
}

/// Run `cargo llvm-cov` for `scope` and return the line-coverage percentage.
/// Returns `Ok(None)` in dry-run mode (nothing was actually measured).
pub fn collect_percent(ctx: &Ctx, scope: &CoverageScope) -> Result<Option<f64>, TaskitError> {
    let sh = &ctx.sh;

    if ctx.dry_run {
        match scope {
            CoverageScope::Package(pkg) => {
                taskit_output::taskit_dry!("cargo llvm-cov --locked -p {pkg} --lib --json");
            }
            CoverageScope::Workspace => {
                taskit_output::taskit_dry!("cargo llvm-cov --locked --workspace --json");
            }
        }
        return Ok(None);
    }

    let json = match scope {
        CoverageScope::Package(pkg) => cmd!(sh, "cargo llvm-cov --locked -p {pkg} --lib --json")
            .read()
            .map_err(TaskitError::other)?,
        CoverageScope::Workspace => cmd!(sh, "cargo llvm-cov --locked --workspace --json")
            .read()
            .map_err(TaskitError::other)?,
    };

    let pct = parse_line_coverage(&json)
        .ok_or_else(|| TaskitError::other("failed to parse cargo llvm-cov --json output"))?;
    Ok(Some(pct))
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

    #[test]
    fn collect_percent_dry_run_returns_none() {
        use taskit_types::config::Config;
        use taskit_types::output_format::OutputFormat;
        let ctx = Ctx::new(
            xshell::Shell::new().expect("shell"),
            std::path::PathBuf::from("."),
            Config::default(),
            true,
            OutputFormat::Human,
        );
        let pct =
            collect_percent(&ctx, &CoverageScope::Workspace).expect("dry-run should not error");
        assert!(pct.is_none());
    }

    #[test]
    fn scope_label() {
        assert_eq!(CoverageScope::Package("taskit-core").label(), "taskit-core");
        assert_eq!(CoverageScope::Workspace.label(), "workspace");
    }
}
