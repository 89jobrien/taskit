//! Demonstrates all six output formatters rendering the same pipeline outcome.
//!
//! Run with: cargo run -p taskit-output --example format_outcome

use std::time::Duration;

use taskit_output::{
    DiagnosticFormatter, GithubFormatter, HumanFormatter, JsonFormatter, JunitFormatter,
    OutputFormatter, SarifFormatter,
};
use taskit_types::step::{
    CommandRecord, DiagnosticLevel, DiagnosticRecord, PipelineOutcome, PipelineRunContext,
    StepDiagnosticContext, StepResult, StepStatus,
};

fn sample_outcome() -> PipelineOutcome {
    PipelineOutcome {
        results: vec![
            StepResult {
                name: "fmt --check".into(),
                status: StepStatus::Pass,
                duration: Duration::from_millis(95),
                error: None,
                gate: true,
                diagnostics: vec![],
                context: StepDiagnosticContext {
                    reproduction: Some("taskit fmt --check".into()),
                    commands: vec![CommandRecord {
                        command: "cargo fmt --check --all".into(),
                        success: Some(true),
                        exit_code: Some(0),
                    }],
                    notes: vec![],
                },
            },
            StepResult {
                name: "lint".into(),
                status: StepStatus::Fail,
                duration: Duration::from_millis(3200),
                error: Some("clippy found errors".into()),
                gate: false,
                diagnostics: vec![DiagnosticRecord {
                    rule_id: "clippy::unwrap_used".into(),
                    message: "used `unwrap()` on a `Result`".into(),
                    level: DiagnosticLevel::Warning,
                    file: Some("src/main.rs".into()),
                    line: Some(42),
                    column: Some(14),
                }],
                context: StepDiagnosticContext {
                    reproduction: Some("taskit lint".into()),
                    commands: vec![CommandRecord {
                        command: "cargo clippy --workspace -- -D warnings".into(),
                        success: Some(false),
                        exit_code: Some(1),
                    }],
                    notes: vec![],
                },
            },
        ],
        total: Duration::from_millis(3295),
        passed: false,
        context: Some(PipelineRunContext {
            taskit_version: "0.7.0".into(),
            workspace_root: "/home/user/myproject".into(),
            rustc_version: Some("1.87.0".into()),
            cargo_version: Some("1.87.0".into()),
            git_sha: Some("c0ffee42".into()),
            taskit_binary: None,
            workspace_members: vec!["myproject".into()],
        }),
    }
}

fn section(title: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {title}");
    println!("{}", "=".repeat(60));
}

fn main() {
    let outcome = sample_outcome();

    section("Human");
    print!("{}", HumanFormatter.render(&outcome));

    section("JSON");
    println!("{}", JsonFormatter.render(&outcome));

    section("GitHub Actions");
    print!("{}", GithubFormatter.render(&outcome));

    section("JUnit XML");
    print!("{}", JunitFormatter.render(&outcome));

    section("Diagnostic (miette)");
    print!("{}", DiagnosticFormatter.render(&outcome));

    section("SARIF 2.1.0");
    println!("{}", SarifFormatter.render(&outcome));
}
