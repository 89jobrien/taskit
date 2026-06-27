use serde::Serialize;

use crate::step::{PipelineOutcome, StepStatus};

/// Output format for pipeline results.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Github,
    Junit,
}

/// Port: formats pipeline results for different output targets.
pub trait OutputFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}

impl OutputFormat {
    pub fn formatter(self) -> Box<dyn OutputFormatter> {
        match self {
            OutputFormat::Human => Box::new(HumanFormatter),
            OutputFormat::Json => Box::new(JsonFormatter),
            OutputFormat::Github => Box::new(GithubFormatter),
            OutputFormat::Junit => Box::new(JunitFormatter),
        }
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

// -- Miette error reporting --------------------------------------------------

use miette::{Diagnostic, NamedSource, SourceSpan};
use std::fmt;

/// A rich diagnostic error emitted when pipeline steps fail.
///
/// Integrates with miette to produce colorized, annotated error output
/// showing which steps failed and their error messages.
#[derive(Debug, Diagnostic)]
#[diagnostic(
    code(taskit::pipeline_failed),
    help("fix the failing steps above, then re-run")
)]
pub struct PipelineError {
    #[source_code]
    src: NamedSource<String>,
    #[label("pipeline result")]
    span: SourceSpan,
    #[related]
    failures: Vec<StepError>,
}

#[derive(Debug, Diagnostic)]
#[diagnostic(severity(error))]
pub struct StepError {
    step_name: String,
    #[help]
    detail: Option<String>,
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step \"{}\" failed", self.step_name)
    }
}

impl std::error::Error for StepError {}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pipeline failed: {} step(s) failed", self.failures.len())
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Build a miette-compatible PipelineError from a failed outcome.
pub fn pipeline_error(outcome: &PipelineOutcome) -> PipelineError {
    let summary = render_summary_text(outcome);
    let len = summary.len();
    let failures: Vec<StepError> = outcome
        .results
        .iter()
        .filter(|s| s.status == StepStatus::Fail)
        .map(|s| StepError {
            step_name: s.name.clone(),
            detail: s.error.clone(),
        })
        .collect();
    PipelineError {
        src: NamedSource::new("pipeline-summary", summary),
        span: (0, len).into(),
        failures,
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
    let formatter = format.formatter();
    let rendered = formatter.render(outcome);
    match format {
        OutputFormat::Json => print!("{rendered}"),
        OutputFormat::Junit => {
            let path = "target/taskit-results.xml";
            std::fs::write(path, &rendered).ok();
            eprintln!("JUnit results written to {path}");
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
                },
                StepResult {
                    name: "test".into(),
                    status: StepStatus::Fail,
                    duration: Duration::from_millis(14700),
                    error: Some("3 tests failed".into()),
                    gate: false,
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
        assert_eq!(err.failures.len(), 1);
        assert_eq!(err.failures[0].step_name, "test");
    }

    #[test]
    fn pipeline_error_display() {
        let err = pipeline_error(&sample_outcome());
        assert!(err.to_string().contains("1 step(s) failed"));
    }

    #[test]
    fn step_error_display() {
        let err = StepError {
            step_name: "lint".into(),
            detail: Some("clippy warnings".into()),
        };
        assert!(err.to_string().contains("lint"));
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
