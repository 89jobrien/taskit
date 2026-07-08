//! Demonstrates the `Pipeline` builder from taskit-engine.
//!
//! Shows steps, gates, context sinks, fail-fast behaviour, and how
//! `PipelineRunContext` is attached to the outcome.
//!
//! Run with: cargo run -p taskit-engine --example pipeline_builder

use std::cell::RefCell;
use std::rc::Rc;

use taskit_engine::step::{Pipeline, StepContextSink, StepStatus};
use taskit_types::error::TaskitError;
use taskit_types::step::{PipelineRunContext, StepDiagnosticContext};

fn main() {
    println!("=== fail_fast=false: all steps run ===\n");
    run_demo(false);

    println!("\n=== fail_fast=true: stops after first failure ===\n");
    run_demo(true);

    println!("\n=== context sinks: capture reproduction + commands ===\n");
    run_context_demo();
}

fn run_demo(fail_fast: bool) {
    let outcome = Pipeline::new(fail_fast)
        // Gates block all subsequent steps on failure regardless of fail_fast.
        .gate("preflight", || {
            println!("  preflight: checking tools...");
            Ok(())
        })
        .step("fmt", || {
            println!("  fmt: running cargo fmt --check");
            Ok(())
        })
        .step("lint", || {
            println!("  lint: running clippy");
            Err(TaskitError::other("clippy found 1 warning"))
        })
        .step("test", || {
            println!("  test: running nextest");
            Ok(())
        })
        .run();

    for s in &outcome.results {
        println!("  [{:4}] {}", s.status, s.name);
        if s.status == StepStatus::Skipped {
            println!("         (skipped due to prior failure or gate)");
        }
    }
    println!("  passed: {}", outcome.passed);
}

fn run_context_demo() {
    // Each step gets its own sink so it can carry reproduction hints and
    // recorded commands into the outcome independent of pass/fail status.
    let lint_sink: StepContextSink = Rc::new(RefCell::new(StepDiagnosticContext {
        reproduction: Some("taskit lint".into()),
        ..Default::default()
    }));

    let outcome = Pipeline::new(false)
        .with_context(PipelineRunContext {
            taskit_version: "0.7.0".into(),
            workspace_root: std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            ..Default::default()
        })
        .step_with_context_sink("lint", lint_sink, || {
            Err(TaskitError::other("clippy found errors"))
        })
        .run();

    for s in &outcome.results {
        println!("  [{:4}] {}", s.status, s.name);
        if let Some(ref repro) = s.context.reproduction {
            println!("         reproduction: {repro}");
        }
        if let Some(ref e) = s.error {
            println!("         error: {e}");
        }
    }

    if let Some(ref ctx) = outcome.context {
        println!("  run context: taskit {}", ctx.taskit_version);
        println!("  workspace:   {}", ctx.workspace_root);
    }
}
