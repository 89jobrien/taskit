//! Demonstrates implementing the `PipelineRunner` port from taskit-core.
//!
//! A custom runner lets you swap in any pipeline backend (embedded scripting,
//! remote execution, dry-run mocking) without changing the rest of taskit.
//!
//! Run with: cargo run -p taskit-core --example custom_runner

use std::path::Path;
use std::time::{Duration, Instant};

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_types::error::TaskitError;
use taskit_types::step::{PipelineOutcome, StepDiagnosticContext, StepResult, StepStatus};

/// A dry-run runner that records what it *would* do without executing anything.
struct DryRunRunner {
    steps: Vec<&'static str>,
}

impl PipelineRunner for DryRunRunner {
    fn run_pipeline(
        &self,
        config_path: &Path,
        _fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        eprintln!("dry-run: config = {}", config_path.display());

        let start = Instant::now();
        let results: Vec<StepResult> = self
            .steps
            .iter()
            .map(|name| {
                eprintln!("dry-run: would run step \"{name}\"");
                StepResult {
                    name: (*name).to_string(),
                    status: StepStatus::Skipped,
                    duration: Duration::ZERO,
                    error: None,
                    gate: false,
                    diagnostics: vec![],
                    context: StepDiagnosticContext {
                        reproduction: Some(format!("taskit {name}")),
                        ..Default::default()
                    },
                }
            })
            .collect();

        Ok(PipelineOutcome {
            total: start.elapsed(),
            passed: true,
            results,
            context: None,
        })
    }
}

fn main() {
    let runner = DryRunRunner {
        steps: vec!["fmt --check", "lint", "test", "check-deps"],
    };

    let outcome = runner
        .run_pipeline(Path::new("taskit.toml"), false)
        .expect("dry-run never fails");

    println!("\nDry-run outcome (passed={})", outcome.passed);
    for s in &outcome.results {
        let repro = s
            .context
            .reproduction
            .as_deref()
            .unwrap_or("(no reproduction)");
        println!("  [{:4}] {}  →  {repro}", s.status, s.name);
    }
}
