use std::cell::RefCell;
use std::rc::Rc;

use taskit_types::error::TaskitError;
use taskit_types::output_format::OutputFormat;
use taskit_types::step::StepDiagnosticContext;

use crate::{
    check_deps,
    config::CiConfig,
    ctx::Ctx,
    dev_setup, fmt, lint, protocol,
    step::{Pipeline, PipelineOutcome, StepContextSink},
    testing,
};

/// Map a command string to its `taskit <cmd>` reproduction form.
fn reproduction_for_cmd(cmd: &str) -> String {
    format!("taskit {cmd}")
}

/// Create a pre-loaded `StepContextSink` and wrap `f` to bracket Ctx command capture.
///
/// The returned sink is pre-populated with `reproduction`. When the wrapper runs, it
/// brackets `f` with `ctx.command_capture_start/finish` and drains the recorded commands
/// into the sink.
fn with_capture<'a>(
    ctx: &'a Ctx,
    reproduction: String,
    f: impl FnOnce() -> Result<(), TaskitError> + 'a,
) -> (
    StepContextSink,
    impl FnOnce() -> Result<(), TaskitError> + 'a,
) {
    let sink: StepContextSink = Rc::new(RefCell::new(StepDiagnosticContext {
        reproduction: Some(reproduction),
        ..StepDiagnosticContext::default()
    }));
    let sink_clone = sink.clone();
    let wrapped = move || {
        let start = ctx.command_capture_start();
        let result = f();
        let commands = ctx.command_capture_finish(start);
        sink_clone.borrow_mut().commands = commands;
        result
    };
    (sink, wrapped)
}

/// Run the CI pipeline.
///
/// When `[ci]` contains steps they are dispatched dynamically from the config,
/// allowing workspaces to define their own pipeline in `taskit.toml`.
/// When `[ci]` is absent or empty the built-in default pipeline is used.
pub fn run(ctx: &Ctx, fail_fast: bool, include_network: bool) -> Result<(), TaskitError> {
    let offline = !include_network;
    let output_format = ctx.output;
    let capture = matches!(output_format, OutputFormat::Sarif);
    let outcome = match ctx.ci() {
        Some(cfg) if !cfg.steps.is_empty() => {
            run_from_config_internal(ctx, cfg, fail_fast, offline)
        }
        Some(_) => {
            // Explicit [ci] with empty steps = run nothing
            Pipeline::new(fail_fast).run()
        }
        None => run_default_pipeline(ctx, fail_fast, offline, capture),
    };
    Ok(taskit_output::write_output(output_format, &outcome)?)
}

/// Build and run a pipeline from `[[ci.steps]]` in `taskit.toml`.
pub(crate) fn run_from_config_internal(
    ctx: &Ctx,
    cfg: &CiConfig,
    fail_fast: bool,
    offline: bool,
) -> PipelineOutcome {
    // Build pipeline; if dispatch fails, return a single-step failure outcome.
    let mut pipeline = Pipeline::new(fail_fast).with_context(ctx.pipeline_run_context());
    for step in &cfg.steps {
        let f = match dispatch_cmd(&step.cmd, ctx, offline) {
            Ok(f) => f,
            Err(e) => {
                return PipelineOutcome {
                    results: vec![crate::step::StepResult {
                        name: step.name.clone(),
                        status: crate::step::StepStatus::Fail,
                        duration: std::time::Duration::ZERO,
                        error: Some(e.to_string()),
                        gate: step.gate,
                        diagnostics: vec![],
                        context: Default::default(),
                    }],
                    total: std::time::Duration::ZERO,
                    passed: false,
                    context: None,
                };
            }
        };
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd(&step.cmd), f);
        if step.gate {
            pipeline = pipeline.gate_with_context_sink(&step.name, sink, wrapped);
        } else {
            pipeline = pipeline.step_with_context_sink(&step.name, sink, wrapped);
        }
    }
    pipeline.run()
}

/// Map a `cmd` string to a closure that runs the corresponding built-in step.
///
/// The `cmd` syntax mirrors taskit's CLI subcommands:
/// `"fmt --check"`, `"lint"`, `"test"`, `"coverage"`, `"compile-tests"`,
/// `"check-deps"`, `"check-protocol-drift"`, `"self-check"`.
fn dispatch_cmd<'a>(
    cmd: &str,
    ctx: &'a Ctx,
    offline: bool,
) -> Result<Box<dyn FnOnce() -> Result<(), TaskitError> + 'a>, TaskitError> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let sub = *parts.first().unwrap_or(&"");
    let f: Box<dyn FnOnce() -> Result<(), TaskitError> + 'a> = match sub {
        "fmt" => {
            let check = parts.contains(&"--check");
            Box::new(move || fmt::run(ctx, check, false))
        }
        "lint" => Box::new(move || lint::run(ctx, None, false, false)),
        "compile-tests" => Box::new(move || testing::compile::run(ctx)),
        "test" => Box::new(move || testing::run::run(ctx, None, false, false, offline)),
        "coverage" => Box::new(move || match ctx.cov() {
            Some(c) => testing::coverage::run(ctx, &c.crate_name, c.threshold()),
            None => {
                taskit_output::taskit_skip!("coverage: skipped (no [coverage] in taskit.toml)");
                Ok(())
            }
        }),
        "check-deps" => Box::new(move || check_deps::run(ctx)),
        "check-protocol-drift" => Box::new(move || protocol::drift::run(ctx, false, false, false)),
        "self-check" => Box::new(dev_setup::self_check),
        "health" => Box::new(move || crate::health::run(ctx, false)),
        other => {
            return Err(TaskitError::other(format!(
                "unknown ci step command: {other:?}"
            )));
        }
    };
    Ok(f)
}

/// The built-in default pipeline, used when no `[[ci.steps]]` are configured.
pub(crate) fn run_default_internal(ctx: &Ctx, fail_fast: bool, offline: bool) -> PipelineOutcome {
    run_default_pipeline(ctx, fail_fast, offline, false)
}

/// Build and run the default pipeline, optionally capturing per-diagnostic data.
fn run_default_pipeline(
    ctx: &Ctx,
    fail_fast: bool,
    offline: bool,
    capture_diagnostics: bool,
) -> PipelineOutcome {
    use crate::step::DiagnosticSink;

    let mut pipeline = Pipeline::new(fail_fast).with_context(ctx.pipeline_run_context());

    // self-check gate
    {
        let (sink, wrapped) = with_capture(
            ctx,
            reproduction_for_cmd("self-check"),
            dev_setup::self_check,
        );
        pipeline = pipeline.gate_with_context_sink("self-check", sink, wrapped);
    }

    // fmt --check
    {
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd("fmt --check"), || {
            fmt::run(ctx, true, false)
        });
        pipeline = pipeline.step_with_context_sink("fmt --check", sink, wrapped);
    }

    // lint
    if capture_diagnostics {
        let lint_diag_sink: DiagnosticSink = Rc::new(RefCell::new(Vec::new()));
        let lint_diag_clone = lint_diag_sink.clone();
        let (ctx_sink, wrapped) = with_capture(ctx, reproduction_for_cmd("lint"), move || {
            let (success, diags) = lint::run_capturing(ctx)?;
            lint_diag_clone.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("clippy found errors"))
            }
        });
        pipeline = pipeline.step_with_diagnostics_and_context_sink(
            "lint",
            lint_diag_sink,
            ctx_sink,
            wrapped,
        );
    } else {
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd("lint"), || {
            lint::run(ctx, None, false, false)
        });
        pipeline = pipeline.step_with_context_sink("lint", sink, wrapped);
    }

    // compile-tests
    {
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd("compile-tests"), || {
            testing::compile::run(ctx)
        });
        pipeline = pipeline.step_with_context_sink("compile-tests", sink, wrapped);
    }

    // test
    if capture_diagnostics {
        let test_diag_sink: DiagnosticSink = Rc::new(RefCell::new(Vec::new()));
        let test_diag_clone = test_diag_sink.clone();
        let (ctx_sink, wrapped) = with_capture(ctx, reproduction_for_cmd("test"), move || {
            let (success, diags) = testing::run::run_capturing(ctx, offline)?;
            test_diag_clone.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("tests failed"))
            }
        });
        pipeline = pipeline.step_with_diagnostics_and_context_sink(
            "test",
            test_diag_sink,
            ctx_sink,
            wrapped,
        );
    } else {
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd("test"), || {
            testing::run::run(ctx, None, false, false, offline)
        });
        pipeline = pipeline.step_with_context_sink("test", sink, wrapped);
    }

    // check-deps
    {
        let (sink, wrapped) = with_capture(ctx, reproduction_for_cmd("check-deps"), || {
            check_deps::run(ctx)
        });
        pipeline = pipeline.step_with_context_sink("check-deps", sink, wrapped);
    }

    // check-protocol-drift
    {
        let (sink, wrapped) =
            with_capture(ctx, reproduction_for_cmd("check-protocol-drift"), || {
                protocol::drift::run(ctx, false, false, false)
            });
        pipeline = pipeline.step_with_context_sink("check-protocol-drift", sink, wrapped);
    }

    // coverage (optional)
    if let Some(c) = ctx.cov() {
        let crate_name = c.crate_name.clone();
        let threshold = c.threshold();
        let step_name = format!("coverage ({crate_name})");
        let repro = format!("taskit coverage --crate-name {crate_name}");
        let (sink, wrapped) = with_capture(ctx, repro, move || {
            testing::coverage::run(ctx, &crate_name, threshold)
        });
        pipeline = pipeline.step_with_context_sink(&step_name, sink, wrapped);
    }

    pipeline.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::Ctx;

    // --- dispatch_cmd ---

    #[test]
    fn dispatch_cmd_unknown_returns_error() {
        let ctx = Ctx::test();
        match dispatch_cmd("frobnicate", &ctx, false) {
            Err(e) => assert!(
                e.to_string().contains("unknown ci step command"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected Err for unknown command"),
        }
    }

    #[test]
    fn dispatch_cmd_empty_string_returns_error() {
        let ctx = Ctx::test();
        assert!(dispatch_cmd("", &ctx, false).is_err());
    }

    #[test]
    fn dispatch_cmd_known_cmds_return_ok() {
        let ctx = Ctx::test();
        let known = [
            "fmt",
            "fmt --check",
            "lint",
            "compile-tests",
            "test",
            "coverage",
            "check-deps",
            "check-protocol-drift",
            "self-check",
            "health",
        ];
        for cmd in known {
            assert!(
                dispatch_cmd(cmd, &ctx, false).is_ok(),
                "dispatch_cmd({cmd:?}) should return Ok"
            );
        }
    }

    #[test]
    fn dispatch_cmd_fmt_check_flag_parsed() {
        let ctx = Ctx::test();
        assert!(dispatch_cmd("fmt --check", &ctx, false).is_ok());
        assert!(dispatch_cmd("fmt", &ctx, false).is_ok());
    }

    // --- run_from_config with empty steps ---

    #[test]
    fn empty_steps_runs_nothing() {
        let ctx = Ctx::test();
        let cfg = CiConfig {
            steps: vec![],
            cruxfile: None,
        };
        // Empty steps = run nothing (not the default pipeline)
        let outcome = run_from_config_internal(&ctx, &cfg, false, false);
        assert!(outcome.passed);
        assert!(outcome.results.is_empty());
    }
}
