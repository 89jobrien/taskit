// TODO(audit): 1,251 lines — largest module in the workspace, split candidate.
use miette::NamedSource;
use serde::Serialize;

use taskit_types::error::{PipelineError, StepError};
use taskit_types::output_format::OutputFormat;
use taskit_types::step::{
    DiagnosticLevel, PipelineOutcome, PipelineRunContext, StepDiagnosticContext, StepResult,
    StepStatus,
};

/// Port: formats pipeline results for different output targets.
pub trait OutputFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}

pub fn formatter_for(format: OutputFormat) -> Box<dyn OutputFormatter> {
    match format {
        OutputFormat::Human => Box::new(HumanFormatter),
        OutputFormat::Compact => Box::new(CompactFormatter {
            verbose_on_failure: true,
        }),
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

        // Append per-step context for failed steps.
        let failed: Vec<&StepResult> = outcome
            .results
            .iter()
            .filter(|s| s.status == StepStatus::Fail)
            .collect();
        if !failed.is_empty() {
            out.push('\n');
            for s in &failed {
                let ctx_text = render_step_context_text(&s.name, &s.context);
                if !ctx_text.is_empty() {
                    out.push_str(&format!("{}:\n", s.name));
                    out.push_str(&ctx_text);
                }
            }
        }

        // Run context footer.
        if let Some(ref ctx) = outcome.context {
            out.push('\n');
            out.push_str(&render_run_context_text(ctx));
        }

        out
    }
}

// -- Compact -----------------------------------------------------------------

pub struct CompactFormatter {
    pub verbose_on_failure: bool,
}

impl OutputFormatter for CompactFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let mut out = String::new();
        for s in &outcome.results {
            let icon = match s.status {
                StepStatus::Pass => "✓",
                StepStatus::Fail => "✗",
                StepStatus::Skipped => "-",
            };
            out.push_str(&format!(
                "{icon} {} ({:.1}s)\n",
                s.name,
                s.duration.as_secs_f64()
            ));
            if self.verbose_on_failure && s.status == StepStatus::Fail {
                if let Some(ref err) = s.error {
                    out.push_str(&format!("  {err}\n"));
                }
                if let Some(ref repro) = s.context.reproduction {
                    out.push_str(&format!("  → {repro}\n"));
                }
            }
        }
        if out.is_empty() {
            out.push_str("(no steps)\n");
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    run_context: Option<JsonRunContext>,
}

#[derive(Serialize)]
struct JsonStep {
    name: String,
    status: String,
    duration_secs: f64,
    error: Option<String>,
    gate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<JsonStepContext>,
}

#[derive(Serialize)]
struct JsonStepContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    reproduction: Option<String>,
    commands: Vec<JsonCommandRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

#[derive(Serialize)]
struct JsonCommandRecord {
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}

#[derive(Serialize)]
struct JsonRunContext {
    taskit_version: String,
    workspace_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    git_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rustc_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cargo_version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    workspace_members: Vec<String>,
}

fn json_step_context(ctx: &StepDiagnosticContext) -> Option<JsonStepContext> {
    if ctx.reproduction.is_none() && ctx.commands.is_empty() && ctx.notes.is_empty() {
        return None;
    }
    Some(JsonStepContext {
        reproduction: ctx.reproduction.clone(),
        commands: ctx
            .commands
            .iter()
            .map(|c| JsonCommandRecord {
                command: c.command.clone(),
                success: c.success,
                exit_code: c.exit_code,
            })
            .collect(),
        notes: ctx.notes.clone(),
    })
}

pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String {
        let run_context = outcome.context.as_ref().map(|ctx| JsonRunContext {
            taskit_version: ctx.taskit_version.clone(),
            workspace_root: ctx.workspace_root.clone(),
            git_sha: ctx.git_sha.clone(),
            rustc_version: ctx.rustc_version.clone(),
            cargo_version: ctx.cargo_version.clone(),
            workspace_members: ctx.workspace_members.clone(),
        });
        let output = JsonOutput {
            version: 2,
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
                    context: json_step_context(&s.context),
                })
                .collect(),
            total_duration_secs: outcome.total.as_secs_f64(),
            passed: outcome.passed,
            run_context,
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
                    let repro = s
                        .context
                        .reproduction
                        .as_deref()
                        .map(|r| format!(" | Reproduction: {r}"))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "::error title={}::Step \"{}\" failed ({:.1}s): {}{}\n",
                        s.name,
                        s.name,
                        s.duration.as_secs_f64(),
                        msg,
                        repro,
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

        // Summary table (also written to GITHUB_STEP_SUMMARY by write_output).
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

        // Reproduction table for failed steps.
        let failed_with_repro: Vec<(&str, &str)> = outcome
            .results
            .iter()
            .filter(|s| s.status == StepStatus::Fail)
            .filter_map(|s| {
                s.context
                    .reproduction
                    .as_deref()
                    .map(|r| (s.name.as_str(), r))
            })
            .collect();
        if !failed_with_repro.is_empty() {
            out.push_str("\n## Reproduction\n\n");
            out.push_str("| Step | Command |\n");
            out.push_str("|---|---|\n");
            for (name, repro) in &failed_with_repro {
                out.push_str(&format!("| {name} | `{repro}` |\n"));
            }
        }

        // Run context section.
        if let Some(ref ctx) = outcome.context {
            out.push_str("\n## Run context\n\n");
            out.push_str(&format!("- taskit {}\n", ctx.taskit_version));
            if let Some(ref v) = ctx.rustc_version {
                out.push_str(&format!("- rustc {v}\n"));
            }
            if let Some(ref v) = ctx.cargo_version {
                out.push_str(&format!("- cargo {v}\n"));
            }
            if let Some(ref sha) = ctx.git_sha {
                out.push_str(&format!("- git {}\n", &sha[..sha.len().min(8)]));
            }
            out.push_str(&format!("- workspace: `{}`\n", ctx.workspace_root));
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
                    if let Some(ref repro) = s.context.reproduction {
                        out.push_str(&format!(
                            "      <system-out>{}</system-out>\n",
                            xml_escape(repro)
                        ));
                    }
                    out.push_str("    </testcase>\n");
                }
                StepStatus::Skipped => {
                    out.push_str(&format!(
                        "    <testcase name=\"{}\" time=\"0.0\">\n      \
                         <skipped/>\n    </testcase>\n",
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

            // Append per-step reproduction hints for failed steps.
            let mut hints = String::new();
            for s in outcome
                .results
                .iter()
                .filter(|s| s.status == StepStatus::Fail)
            {
                let ctx_text = render_step_context_text(&s.name, &s.context);
                if !ctx_text.is_empty() {
                    hints.push_str(&format!("{}:\n", s.name));
                    hints.push_str(&ctx_text);
                }
            }
            if !hints.is_empty() {
                buf.push('\n');
                buf.push_str(&hints);
            }

            if let Some(ref ctx) = outcome.context {
                buf.push('\n');
                buf.push_str(&render_run_context_text(ctx));
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
            "$schema": format!(
                "https://{}/{}/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
                "raw.githubusercontent.com", "oasis-tcs"
            ),
            "version": "2.1.0",
            "runs": runs,
        });
        serde_json::to_string_pretty(&sarif).expect("SARIF serialization cannot fail")
    }
}

fn sarif_run_for_step(step: &StepResult) -> serde_json::Value {
    let tool_name = sarif_tool_name(&step.name);
    let execution_successful = step.status != StepStatus::Fail;

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

    let mut invocation = serde_json::json!({
        "executionSuccessful": execution_successful,
    });
    if let Some(ref repro) = step.context.reproduction {
        invocation["properties"] = serde_json::json!({ "reproduction": repro });
    }

    serde_json::json!({
        "tool": { "driver": driver },
        "invocations": [invocation],
        "results": results,
    })
}

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
    "taskit"
}

// -- Context rendering helpers -----------------------------------------------

/// Render the reproduction hint + command log for a failed step (plain text).
fn render_step_context_text(name: &str, ctx: &StepDiagnosticContext) -> String {
    let mut out = String::new();
    if let Some(repro) = &ctx.reproduction {
        out.push_str(&format!("  Reproduction: {repro}\n"));
    }
    if !ctx.commands.is_empty() {
        out.push_str(&format!("  Commands run during \"{name}\":\n"));
        for cmd in &ctx.commands {
            let status = match cmd.success {
                Some(true) => "ok",
                Some(false) => "fail",
                None => "?",
            };
            out.push_str(&format!("    [{}] {}\n", status, cmd.command));
        }
    }
    for note in &ctx.notes {
        out.push_str(&format!("  Note: {note}\n"));
    }
    out
}

/// Render PipelineRunContext as a compact footer line.
fn render_run_context_text(ctx: &PipelineRunContext) -> String {
    let mut parts: Vec<String> = vec![format!("taskit {}", ctx.taskit_version)];
    if let Some(ref v) = ctx.rustc_version {
        parts.push(format!("rustc {v}"));
    }
    if let Some(ref v) = ctx.cargo_version {
        parts.push(format!("cargo {v}"));
    }
    if let Some(ref sha) = ctx.git_sha {
        parts.push(format!("git {}", &sha[..sha.len().min(8)]));
    }
    let mut out = format!("Run context: {}\n", parts.join(" | "));
    out.push_str(&format!("  workspace: {}\n", ctx.workspace_root));
    out
}

// -- Miette error reporting --------------------------------------------------

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
        OutputFormat::Compact => eprint!("{rendered}"),
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
                    context: Default::default(),
                },
                StepResult {
                    name: "test".into(),
                    status: StepStatus::Fail,
                    duration: Duration::from_millis(14700),
                    error: Some("3 tests failed".into()),
                    gate: false,
                    diagnostics: vec![],
                    context: Default::default(),
                },
            ],
            total: Duration::from_millis(15900),
            passed: false,
            context: None,
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
                context: Default::default(),
            }],
            total: Duration::from_secs(1),
            passed: true,
            context: None,
        }
    }

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

    #[test]
    fn json_formatter_valid_json() {
        let output = JsonFormatter.render(&sample_outcome());
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert_eq!(parsed["version"], 2);
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

    #[test]
    fn junit_formatter_valid_xml() {
        let output = JunitFormatter.render(&sample_outcome());
        assert!(output.contains("<testsuites>"));
        assert!(output.contains("</testsuites>"));
        assert!(output.contains("tests=\"2\""));
        assert!(output.contains("failures=\"1\""));
        assert!(output.contains("<failure"));
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
                context: Default::default(),
            }],
            total: Duration::ZERO,
            passed: true,
            context: None,
        };
        let output = JunitFormatter.render(&outcome);
        assert!(output.contains("<skipped/>"));
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("a<b>c&d\"e"), "a&lt;b&gt;c&amp;d&quot;e");
    }

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

    #[test]
    fn write_output_returns_ok_for_passing() {
        assert!(write_output(OutputFormat::Human, &passing_outcome()).is_ok());
    }

    #[test]
    fn write_output_returns_err_for_failing() {
        assert!(write_output(OutputFormat::Human, &sample_outcome()).is_err());
    }

    // -- Conformance tests ---------------------------------------------------
    //
    // Each formatter must satisfy five invariants regardless of which concrete
    // type is under test.  `assert_formatter_contract` encodes those invariants
    // and is called once per implementation.

    const STEP_NAME: &str = "compile";

    fn one_step_passing() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![StepResult {
                name: STEP_NAME.into(),
                status: StepStatus::Pass,
                duration: Duration::from_millis(500),
                error: None,
                gate: true,
                diagnostics: vec![],
                context: Default::default(),
            }],
            total: Duration::from_millis(500),
            passed: true,
            context: None,
        }
    }

    fn one_step_failing() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![StepResult {
                name: STEP_NAME.into(),
                status: StepStatus::Fail,
                duration: Duration::from_millis(200),
                error: Some("exit code 1".into()),
                gate: true,
                diagnostics: vec![],
                context: Default::default(),
            }],
            total: Duration::from_millis(200),
            passed: false,
            context: None,
        }
    }

    fn empty_outcome() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![],
            total: Duration::ZERO,
            passed: true,
            context: None,
        }
    }

    fn long_name_outcome() -> PipelineOutcome {
        let name = "x".repeat(1000);
        PipelineOutcome {
            results: vec![StepResult {
                name: name.clone(),
                status: StepStatus::Pass,
                duration: Duration::from_millis(1),
                error: None,
                gate: false,
                diagnostics: vec![],
                context: Default::default(),
            }],
            total: Duration::from_millis(1),
            passed: true,
            context: None,
        }
    }

    /// Assert the five invariants that every `OutputFormatter` implementation must satisfy.
    ///
    /// `expected_step_token` is the string that must appear in the render of
    /// `one_step_passing()`.  For most formatters this is the literal step name
    /// (`STEP_NAME`); for SARIF the step name is mapped to a derived tool name
    /// by `sarif_tool_name`, so the expected token differs.
    fn assert_formatter_contract(formatter: &dyn OutputFormatter, expected_step_token: &str) {
        // Invariant 1: non-empty output for a passing outcome.
        let out = formatter.render(&one_step_passing());
        assert!(!out.is_empty(), "render(passing) must not be empty");

        // Invariant 2: non-empty output for a failing outcome.
        let out = formatter.render(&one_step_failing());
        assert!(!out.is_empty(), "render(failing) must not be empty");

        // Invariant 3: non-empty output for an outcome with zero steps.
        let out = formatter.render(&empty_outcome());
        assert!(!out.is_empty(), "render(empty) must not be empty");

        // Invariant 4: no panic on a very long step name.
        let _out = formatter.render(&long_name_outcome());

        // Invariant 5: passing outcome output must contain the expected token
        // derived from the step name.
        let out = formatter.render(&one_step_passing());
        assert!(
            out.contains(expected_step_token),
            "render(passing) must contain \"{expected_step_token}\""
        );
    }

    #[test]
    fn human_formatter_contract() {
        assert_formatter_contract(&HumanFormatter, STEP_NAME);
    }

    #[test]
    fn json_formatter_contract() {
        assert_formatter_contract(&JsonFormatter, STEP_NAME);
    }

    #[test]
    fn github_formatter_contract() {
        assert_formatter_contract(&GithubFormatter, STEP_NAME);
    }

    #[test]
    fn junit_formatter_contract() {
        assert_formatter_contract(&JunitFormatter, STEP_NAME);
    }

    #[test]
    fn diagnostic_formatter_contract() {
        assert_formatter_contract(&DiagnosticFormatter, STEP_NAME);
    }

    #[test]
    fn sarif_formatter_contract() {
        // STEP_NAME ("compile") does not match any keyword in sarif_tool_name,
        // so the derived tool name is the fallback "taskit".
        assert_formatter_contract(&SarifFormatter, "taskit");
    }

    #[test]
    fn compact_formatter_contract() {
        assert_formatter_contract(
            &CompactFormatter {
                verbose_on_failure: true,
            },
            STEP_NAME,
        );
    }

    #[test]
    fn compact_formatter_shows_icon_and_duration() {
        let out = CompactFormatter {
            verbose_on_failure: false,
        }
        .render(&sample_outcome());
        assert!(out.contains("✓"), "passing step should have ✓");
        assert!(out.contains("✗"), "failing step should have ✗");
        assert!(out.contains("fmt"));
        assert!(out.contains("test"));
    }

    #[test]
    fn compact_formatter_verbose_on_failure_shows_error() {
        let out = CompactFormatter {
            verbose_on_failure: true,
        }
        .render(&sample_outcome());
        assert!(
            out.contains("3 tests failed"),
            "error message should appear"
        );
    }

    #[test]
    fn compact_formatter_no_verbose_hides_error() {
        let out = CompactFormatter {
            verbose_on_failure: false,
        }
        .render(&sample_outcome());
        assert!(!out.contains("3 tests failed"), "error should be hidden");
    }

    #[test]
    fn compact_formatter_empty_outcome_shows_placeholder() {
        let out = CompactFormatter {
            verbose_on_failure: false,
        }
        .render(&empty_outcome());
        assert!(out.contains("(no steps)"));
    }

    // -- Diagnostic context rendering ----------------------------------------

    use taskit_types::step::{CommandRecord, PipelineRunContext, StepDiagnosticContext};

    fn outcome_with_context() -> PipelineOutcome {
        PipelineOutcome {
            results: vec![StepResult {
                name: "test".into(),
                status: StepStatus::Fail,
                duration: Duration::from_secs(5),
                error: Some("tests failed".into()),
                gate: false,
                diagnostics: vec![],
                context: StepDiagnosticContext {
                    reproduction: Some("taskit test".into()),
                    commands: vec![CommandRecord {
                        command: "cargo nextest run".into(),
                        success: Some(false),
                        exit_code: Some(1),
                    }],
                    notes: vec![],
                },
            }],
            total: Duration::from_secs(5),
            passed: false,
            context: Some(PipelineRunContext {
                taskit_version: "0.7.0".into(),
                workspace_root: "/workspace".into(),
                rustc_version: Some("1.87.0".into()),
                cargo_version: Some("1.87.0".into()),
                git_sha: Some("abc12345def".into()),
                ..PipelineRunContext::default()
            }),
        }
    }

    #[test]
    fn human_formatter_renders_reproduction_on_failure() {
        let out = HumanFormatter.render(&outcome_with_context());
        assert!(
            out.contains("taskit test"),
            "should contain reproduction command"
        );
        assert!(
            out.contains("cargo nextest run"),
            "should contain command log"
        );
    }

    #[test]
    fn human_formatter_renders_run_context() {
        let out = HumanFormatter.render(&outcome_with_context());
        assert!(out.contains("0.7.0"), "should contain taskit version");
        assert!(out.contains("/workspace"), "should contain workspace root");
    }

    #[test]
    fn human_formatter_no_context_section_on_pass() {
        let out = HumanFormatter.render(&passing_outcome());
        assert!(
            !out.contains("Reproduction:"),
            "no reproduction on passing step"
        );
    }

    #[test]
    fn github_formatter_includes_reproduction_in_error_annotation() {
        let out = GithubFormatter.render(&outcome_with_context());
        assert!(
            out.contains("Reproduction: taskit test"),
            "::error annotation should include reproduction"
        );
    }

    #[test]
    fn github_formatter_includes_reproduction_table() {
        let out = GithubFormatter.render(&outcome_with_context());
        assert!(
            out.contains("## Reproduction"),
            "should include reproduction section"
        );
        assert!(
            out.contains("`taskit test`"),
            "should include reproduction command"
        );
    }

    #[test]
    fn github_formatter_includes_run_context_section() {
        let out = GithubFormatter.render(&outcome_with_context());
        assert!(
            out.contains("## Run context"),
            "should include run context section"
        );
        assert!(out.contains("0.7.0"), "should include taskit version");
    }

    #[test]
    fn json_formatter_includes_step_context() {
        let out = JsonFormatter.render(&outcome_with_context());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let ctx = &parsed["steps"][0]["context"];
        assert_eq!(ctx["reproduction"], "taskit test");
        assert_eq!(ctx["commands"][0]["command"], "cargo nextest run");
        assert_eq!(ctx["commands"][0]["success"], false);
    }

    #[test]
    fn json_formatter_includes_run_context() {
        let out = JsonFormatter.render(&outcome_with_context());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["run_context"]["taskit_version"], "0.7.0");
        assert_eq!(parsed["run_context"]["workspace_root"], "/workspace");
        assert_eq!(parsed["run_context"]["rustc_version"], "1.87.0");
    }

    #[test]
    fn json_formatter_no_context_key_when_empty() {
        let out = JsonFormatter.render(&passing_outcome());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(
            parsed["steps"][0]["context"].is_null(),
            "no context key for steps without it"
        );
        assert!(
            parsed["run_context"].is_null(),
            "no run_context when absent"
        );
    }

    #[test]
    fn junit_formatter_includes_system_out_on_failure() {
        let out = JunitFormatter.render(&outcome_with_context());
        assert!(out.contains("<system-out>taskit test</system-out>"));
    }

    #[test]
    fn sarif_formatter_includes_reproduction_in_invocation() {
        let out = SarifFormatter.render(&outcome_with_context());
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let props = &parsed["runs"][0]["invocations"][0]["properties"];
        assert_eq!(props["reproduction"], "taskit test");
    }

    #[test]
    fn render_run_context_text_includes_all_fields() {
        let ctx = PipelineRunContext {
            taskit_version: "0.7.0".into(),
            workspace_root: "/ws".into(),
            rustc_version: Some("1.87.0".into()),
            cargo_version: Some("1.87.0".into()),
            git_sha: Some("abc12345".into()),
            ..PipelineRunContext::default()
        };
        let text = render_run_context_text(&ctx);
        assert!(text.contains("0.7.0"));
        assert!(text.contains("rustc 1.87.0"));
        assert!(text.contains("cargo 1.87.0"));
        assert!(text.contains("abc12345"));
        assert!(text.contains("/ws"));
    }

    #[test]
    fn render_step_context_text_empty_when_no_data() {
        let ctx = StepDiagnosticContext::default();
        let text = render_step_context_text("fmt", &ctx);
        assert!(text.is_empty(), "empty context should produce no output");
    }
}
