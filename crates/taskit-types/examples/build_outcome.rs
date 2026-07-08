//! Demonstrates constructing the core pipeline types from taskit-types.
//!
//! Run with: cargo run -p taskit-types --example build_outcome

use std::time::Duration;

use taskit_types::step::{
    CommandRecord, PipelineOutcome, PipelineRunContext, StepDiagnosticContext, StepResult,
    StepStatus,
};

fn main() {
    // A passing step with command provenance.
    let fmt_step = StepResult {
        name: "fmt --check".into(),
        status: StepStatus::Pass,
        duration: Duration::from_millis(120),
        error: None,
        gate: false,
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
    };

    // A failing step.
    let test_step = StepResult {
        name: "test".into(),
        status: StepStatus::Fail,
        duration: Duration::from_millis(8400),
        error: Some("2 tests failed".into()),
        gate: false,
        diagnostics: vec![],
        context: StepDiagnosticContext {
            reproduction: Some("taskit test".into()),
            commands: vec![CommandRecord {
                command: "cargo nextest run --workspace".into(),
                success: Some(false),
                exit_code: Some(1),
            }],
            notes: vec!["run with NEXTEST_PROFILE=ci for verbose output".into()],
        },
    };

    let outcome = PipelineOutcome {
        results: vec![fmt_step, test_step],
        total: Duration::from_millis(8520),
        passed: false,
        context: Some(PipelineRunContext {
            taskit_version: "0.7.0".into(),
            workspace_root: ".".into(),
            rustc_version: Some("1.87.0".into()),
            cargo_version: Some("1.87.0".into()),
            git_sha: None,
            taskit_binary: None,
            workspace_members: vec!["taskit".into(), "taskit-types".into()],
        }),
    };

    println!("passed: {}", outcome.passed);
    println!("steps:  {}", outcome.results.len());
    for s in &outcome.results {
        println!(
            "  [{:4}] {} ({:.1}s)",
            s.status,
            s.name,
            s.duration.as_secs_f64()
        );
        if let Some(ref repro) = s.context.reproduction {
            println!("         reproduction: {repro}");
        }
        for cmd in &s.context.commands {
            let status = match cmd.success {
                Some(true) => "ok",
                Some(false) => "fail",
                None => "?",
            };
            println!("         [{}] {}", status, cmd.command);
        }
        for note in &s.context.notes {
            println!("         note: {note}");
        }
    }
    if let Some(ref ctx) = outcome.context {
        println!(
            "\nrun context: taskit {} | rustc {}",
            ctx.taskit_version,
            ctx.rustc_version.as_deref().unwrap_or("unknown"),
        );
    }
}
