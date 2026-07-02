use serde::Serialize;

use crate::step::{PipelineOutcome, StepResult, StepStatus};

// Re-export from core: the OutputFormatter port lives in taskit-core,
// the concrete adapters live here.
pub use taskit_core::output_formatter::OutputFormatter;
pub use taskit_types::output_format::OutputFormat;

pub fn formatter_for(format: OutputFormat) -> Box<dyn OutputFormatter> {
    match format {
        OutputFormat::Human => Box::new(HumanFormatter),
        OutputFormat::Json => Box::new(JsonFormatter),
        OutputFormat::Github => Box::new(GithubFormatter),
        OutputFormat::Junit => Box::new(JunitFormatter),
        OutputFormat::Diagnostic => Box::new(DiagnosticFormatter),
        OutputFormat::Sarif => Box::new(SarifFormatter),
    }
}

// -- Human -------------------------------------------------------------------

const COL_NAME: usize = 30;
const COL_STATUS: usize = 10;
const SEPARATOR_WIDTH: usize = 55;

pub struct HumanFormatter;

impl OutputFormatter for HumanFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let mut out = String::new();
        out.push('\n');
        out.push_str(&format!(
            "{:<COL_NAME$} {:<COL_STATUS$} Duration\n",
            "Step", "Status"
        ));
        out.push_str(&"-".repeat(SEPARATOR_WIDTH));
        out.push('\n');
        for s in &outcome.results {
            out.push_str(&format!(
                "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s\n",
                s.name,
                s.status,
                s.duration.as_secs_f64()
            ));
        }
        out.push_str(&"-".repeat(SEPARATOR_WIDTH));
        out.push('\n');
        out.push_str(&format!(
            "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s\n",
            "Total",
            "",
            outcome.total.as_secs_f64()
        ));
        out
    }
}

// -- JSON --------------------------------------------------------------------

#[derive(Serialize)]
struct JsonOutput {
    version: u8,
    steps: Vec<JsonStep>,
    total_duration_secs: f64,
    passed: bool,
}

#[derive(Serialize)]
struct JsonStep {
    name: String,
    status: String,
    duration_secs: f64,
    error: Option<String>,
    gate: bool,
}

pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let output = JsonOutput {
            version: 1,
            steps: outcome
                .results
                .iter()
                .map(|s| JsonStep {
                    name: s.name.clone(),
                    status: match s.status {
                        StepStatus::Pass => "pass".into(),
                        StepStatus::Fail => "fail".into(),
                        StepStatus::Skipped => "skip".into(),
                    },
                    duration_secs: s.duration.as_secs_f64(),
                    error: s.error.clone(),
                    gate: s.gate,
                })
                .collect(),
            total_duration_secs: outcome.total.as_secs_f64(),
            passed: outcome.passed,
        };
        serde_json::to_string_pretty(&output).expect("JSON serialization cannot fail")
    }
}

// -- GitHub Actions ----------------------------------------------------------

pub struct GithubFormatter;

impl OutputFormatter for GithubFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let mut out = String::new();
        for s in &outcome.results {
            match s.status {
                StepStatus::Pass => {
                    out.push_str(&format!(
                        "::notice title={}::Step \"{}\" passed ({:.1}s)\n",
                        s.name,
                        s.name,
                        s.duration.as_secs_f64()
                    ));
                }
                StepStatus::Fail => {
                    let msg = s.error.as_deref().unwrap_or("failed");
                    out.push_str(&format!(
                        "::error title={}::Step \"{}\" failed ({:.1}s): {}\n",
                        s.name,
                        s.name,
                        s.duration.as_secs_f64(),
                        msg
                    ));
                }
                StepStatus::Skipped => {
                    out.push_str(&format!(
                        "::notice title={}::Step \"{}\" skipped\n",
                        s.name, s.name
                    ));
                }
            }
        }
        // Markdown summary table
        out.push_str("\n| Step | Status | Duration |\n");
        out.push_str("|---|---|---|\n");
        for s in &outcome.results {
            out.push_str(&format!(
                "| {} | {} | {:.1}s |\n",
                s.name,
                s.status,
                s.duration.as_secs_f64()
            ));
        }
        out
    }
}

// -- JUnit XML ---------------------------------------------------------------

pub struct JunitFormatter;

impl OutputFormatter for JunitFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let failures = outcome
            .results
            .iter()
            .filter(|s| s.status == StepStatus::Fail)
            .count();
        let tests = outcome.results.len();

        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<testsuites>\n");
        out.push_str(&format!(
            "  <testsuite name=\"taskit\" tests=\"{tests}\" \
             failures=\"{failures}\" time=\"{:.1}\">\n",
            outcome.total.as_secs_f64()
        ));
        for s in &outcome.results {
            match s.status {
                StepStatus::Fail => {
                    let msg = xml_escape(s.error.as_deref().unwrap_or("failed"));
                    out.push_str(&format!(
                        "    <testcase name=\"{}\" time=\"{:.1}\">\n",
                        xml_escape(&s.name),
                        s.duration.as_secs_f64()
                    ));
                    out.push_str(&format!("      <failure message=\"{msg}\"/>\n"));
                    out.push_str("    </testcase>\n");
                }
                StepStatus::Skipped => {
                    out.push_str(&format!(
                        "    <testcase name=\"{}\" time=\"0.0\">\n      <skipped/>\n    </testcase>\n",
                        xml_escape(&s.name)
                    ));
                }
                StepStatus::Pass => {
                    out.push_str(&format!(
                        "    <testcase name=\"{}\" time=\"{:.1}\"/>\n",
                        xml_escape(&s.name),
                        s.duration.as_secs_f64()
                    ));
                }
            }
        }
        out.push_str("  </testsuite>\n");
        out.push_str("</testsuites>\n");
        out
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// -- Diagnostic (miette) -----------------------------------------------------

pub struct DiagnosticFormatter;

impl OutputFormatter for DiagnosticFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        if outcome.passed {
            let table = HumanFormatter.render(outcome);
            let n = outcome.results.len();
            let secs = outcome.total.as_secs_f64();
            format!(
                "{table}pipeline passed ({n} step{}, {secs:.1}s)\n",
                if n == 1 { "" } else { "s" }
            )
        } else {
            let err = pipeline_error(outcome);
            let mut buf = String::new();
            let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
            if is_tty {
                use miette::GraphicalReportHandler;
                let handler = GraphicalReportHandler::new();
                let _ = handler.render_report(&mut buf, &err);
            } else {
                use miette::NarratableReportHandler;
                let handler = NarratableReportHandler::new();
                let _ = handler.render_report(&mut buf, &err);
            }
            buf
        }
    }
}

// -- SARIF 2.1.0 -------------------------------------------------------------

pub struct SarifFormatter;

impl OutputFormatter for SarifFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let runs: Vec<serde_json::Value> = outcome.results.iter().map(sarif_run_for_step).collect();
        let sarif = serde_json::json!({
            "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
            "version": "2.1.0",
            "runs": runs,
        });
        serde_json::to_string_pretty(&sarif).expect("SARIF serialization cannot fail")
    }
}

fn sarif_run_for_step(step: &StepResult) -> serde_json::Value {
    use taskit_types::step::DiagnosticLevel;

    let tool_name = sarif_tool_name(&step.name);
    let execution_successful = step.status != StepStatus::Fail;

    // Collect unique rules from diagnostics.
    let mut rule_ids: Vec<String> = step.diagnostics.iter().map(|d| d.rule_id.clone()).collect();
    rule_ids.sort();
    rule_ids.dedup();

    let rules: Vec<serde_json::Value> = rule_ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "shortDescription": { "text": id },
            })
        })
        .collect();

    let results: Vec<serde_json::Value> = step
        .diagnostics
        .iter()
        .map(|d| {
            let level = match d.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Note => "note",
            };
            let mut result = serde_json::json!({
                "ruleId": d.rule_id,
                "level": level,
                "message": { "text": d.message },
            });
            if let Some(file) = &d.file {
                let mut region = serde_json::Map::new();
                if let Some(line) = d.line {
                    region.insert("startLine".into(), serde_json::Value::Number(line.into()));
                }
                if let Some(col) = d.column {
                    region.insert("startColumn".into(), serde_json::Value::Number(col.into()));
                }
                let location = serde_json::json!({
                    "physicalLocation": {
                        "artifactLocation": { "uri": file },
                        "region": region,
                    }
                });
                result["locations"] = serde_json::json!([location]);
            }
            result
        })
        .collect();

    let mut driver = serde_json::json!({
        "name": tool_name,
    });
    if !rules.is_empty() {
        driver["rules"] = serde_json::json!(rules);
    }

    serde_json::json!({
        "tool": { "driver": driver },
        "invocations": [{
            "executionSuccessful": execution_successful,
        }],
        "results": results,
    })
}

/// Map step name to a recognizable SARIF tool name.
fn sarif_tool_name(step_name: &str) -> &str {
    let lower = step_name.to_ascii_lowercase();
    if lower.contains("lint") || lower.contains("clippy") {
        return "clippy";
    }
    if lower.contains("test") {
        return "cargo-nextest";
    }
    if lower.contains("fmt") {
        return "rustfmt";
    }
    if lower.contains("audit") {
        return "cargo-deny";
    }
    // Leak-free: return the input for unknown steps.
    // We can't return &str from a computed String, so use a static fallback.
    "taskit"
}

// -- Miette error reporting --------------------------------------------------

use miette::NamedSource;
use taskit_types::error::{PipelineError, StepError};

/// Build a miette-compatible PipelineError from a failed outcome.
pub fn pipeline_error(outcome: &PipelineOutcome) -> PipelineError {
    let summary = render_summary_text(outcome);
    let len = summary.len();
    let step_errors: Vec<StepError> = outcome
        .results
        .iter()
        .filter(|s| s.status == StepStatus::Fail)
        .map(|s| StepError {
            name: s.name.clone(),
            detail: s.error.clone(),
        })
        .collect();
    let failed_count = step_errors.len();
    PipelineError::Failed {
        failed_count,
        src: NamedSource::new("pipeline-summary", summary),
        span: (0, len).into(),
        step_errors,
    }
}

fn render_summary_text(outcome: &PipelineOutcome) -> String {
    let mut out = String::new();
    for s in &outcome.results {
        let status = match s.status {
            StepStatus::Pass => "PASS",
            StepStatus::Fail => "FAIL",
            StepStatus::Skipped => "SKIP",
        };
        out.push_str(&format!(
            "{} {} ({:.1}s)\n",
            status,
            s.name,
            s.duration.as_secs_f64()
        ));
    }
    out
}

/// Write formatted output to the appropriate destination and return
/// a miette error if the pipeline failed.
pub fn write_output(format: OutputFormat, outcome: &PipelineOutcome) -> Result<(), PipelineError> {
    let formatter = formatter_for(format);
    let rendered = formatter.render(outcome);
    match format {
        OutputFormat::Json => print!("{rendered}"),
        OutputFormat::Junit => {
            let path = "target/taskit-results.xml";
            std::fs::write(path, &rendered).ok();
            eprintln!("JUnit results written to {path}");
        }
        OutputFormat::Sarif => {
            let path = "target/taskit-results.sarif";
            std::fs::write(path, &rendered).ok();
            eprintln!("SARIF results written to {path}");
        }
        _ => eprint!("{rendered}"),
    }
    if let OutputFormat::Github = format
        && let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY")
        && let Some(idx) = rendered.find("\n| Step ")
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary_path)
        {
            let _ = write!(f, "{}", &rendered[idx..]);
        }
    }
    if outcome.passed {
        Ok(())
    } else {
        Err(pipeline_error(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::StepResult;
    use std::time::Duration;

    fn sample_outcome() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![
                StepResult {
                    name: "fmt".into(),
                    status: StepStatus::Pass,
                    duration: Duration::from_millis(1200),
                    error: None,
                    gate: true,
                    diagnostics: vec![],
                },
                StepResult {
                    name: "test".into(),
                    status: StepStatus::Fail,
                    duration: Duration::from_millis(14700),
                    error: Some("3 tests failed".into()),
                    gate: false,
                    diagnostics: vec![],
                },
            ],
            total: Duration::from_millis(15900),
            passed: false,
        }
    }

    fn passing_outcome() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![StepResult {
                name: "fmt".into(),
                status: StepStatus::Pass,
                duration: Duration::from_secs(1),
                error: None,
                gate: false,
                diagnostics: vec![],
            }],
            total: Duration::from_secs(1),
            passed: true,
        }
    }

    // -- Human --

    #[test]
    fn human_formatter_contains_step_names() {
        let output = HumanFormatter.render(&sample_outcome());
        assert!(output.contains("fmt"));
        assert!(output.contains("test"));
        assert!(output.contains("PASS"));
        assert!(output.contains("FAIL"));
    }

    #[test]
    fn human_formatter_contains_total() {
        let output = HumanFormatter.render(&sample_outcome());
        assert!(output.contains("Total"));
    }

    // -- JSON --

    #[test]
    fn json_formatter_valid_json() {
        let output = JsonFormatter.render(&sample_outcome());
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["passed"], false);
        assert_eq!(parsed["steps"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["steps"][0]["name"], "fmt");
        assert_eq!(parsed["steps"][0]["status"], "pass");
        assert_eq!(parsed["steps"][1]["status"], "fail");
        assert_eq!(parsed["steps"][1]["error"], "3 tests failed");
        assert!(parsed["total_duration_secs"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn json_formatter_null_error_for_passing_step() {
        let output = JsonFormatter.render(&sample_outcome());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["steps"][0]["error"].is_null());
    }

    // -- GitHub --

    #[test]
    fn github_formatter_emits_annotations() {
        let output = GithubFormatter.render(&sample_outcome());
        assert!(output.contains("::notice title=fmt::"));
        assert!(output.contains("::error title=test::"));
    }

    #[test]
    fn github_formatter_includes_summary_table() {
        let output = GithubFormatter.render(&sample_outcome());
        assert!(output.contains("| Step "));
        assert!(output.contains("| fmt "));
    }

    // -- JUnit --

    #[test]
    fn junit_formatter_valid_xml() {
        let output = JunitFormatter.render(&sample_outcome());
        assert!(output.contains("<testsuites>"));
        assert!(output.contains("</testsuites>"));
        assert!(output.contains("<testsuite"));
        assert!(output.contains("name=\"taskit\""));
        assert!(output.contains("tests=\"2\""));
        assert!(output.contains("failures=\"1\""));
        assert!(output.contains("<testcase name=\"fmt\""));
        assert!(output.contains("<testcase name=\"test\""));
        assert!(output.contains("<failure"));
        assert!(output.contains("3 tests failed"));
    }

    #[test]
    fn junit_formatter_passing_pipeline_has_zero_failures() {
        let output = JunitFormatter.render(&passing_outcome());
        assert!(output.contains("failures=\"0\""));
        assert!(!output.contains("<failure"));
    }

    #[test]
    fn junit_formatter_skipped_step() {
        let outcome = PipelineOutcome {
            results: vec![StepResult {
                name: "skipped-step".into(),
                status: StepStatus::Skipped,
                duration: Duration::ZERO,
                error: None,
                gate: false,
                diagnostics: vec![],
            }],
            total: Duration::ZERO,
            passed: true,
        };
        let output = JunitFormatter.render(&outcome);
        assert!(output.contains("<skipped/>"));
    }

    // -- XML escape --

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("a<b>c&d\"e"), "a&lt;b&gt;c&amp;d&quot;e");
    }

    // -- Miette error --

    #[test]
    fn pipeline_error_has_correct_failure_count() {
        let err = pipeline_error(&sample_outcome());
        match &err {
            PipelineError::Failed { step_errors, .. } => {
                assert_eq!(step_errors.len(), 1);
                assert_eq!(step_errors[0].name, "test");
            }
            _ => panic!("expected PipelineError::Failed"),
        }
    }

    #[test]
    fn pipeline_error_display() {
        let err = pipeline_error(&sample_outcome());
        assert!(err.to_string().contains("1 step(s) failed"));
    }

    #[test]
    fn step_error_display() {
        let err = StepError {
            name: "lint".into(),
            detail: Some("clippy warnings".into()),
        };
        assert!(err.to_string().contains("lint"));
    }

    // -- Diagnostic --

    #[test]
    fn diagnostic_formatter_success_contains_oneliner() {
        let outcome = passing_outcome();
        let fmt = formatter_for(OutputFormat::Diagnostic);
        let output = fmt.render(&outcome);
        assert!(output.contains("pipeline passed"));
        assert!(output.contains("1 step"));
    }

    #[test]
    fn diagnostic_formatter_success_contains_table() {
        let outcome = passing_outcome();
        let fmt = formatter_for(OutputFormat::Diagnostic);
        let output = fmt.render(&outcome);
        assert!(output.contains("fmt"));
        assert!(output.contains("PASS"));
    }

    #[test]
    fn diagnostic_formatter_failure_contains_diagnostic() {
        let outcome = sample_outcome();
        let fmt = formatter_for(OutputFormat::Diagnostic);
        let output = fmt.render(&outcome);
        assert!(output.contains("pipeline failed"));
        assert!(output.contains("test"));
    }

    // -- write_output --

    #[test]
    fn write_output_returns_ok_for_passing() {
        assert!(write_output(OutputFormat::Human, &passing_outcome()).is_ok());
    }

    #[test]
    fn write_output_returns_err_for_failing() {
        assert!(write_output(OutputFormat::Human, &sample_outcome()).is_err());
    }
}
