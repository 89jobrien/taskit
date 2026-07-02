use taskit_types::error::TaskitError;
use taskit_types::output_format::OutputFormat;

use crate::{
    check_deps,
    config::CiConfig,
    ctx::Ctx,
    dev_setup, fmt, lint, protocol,
    step::{Pipeline, PipelineOutcome},
    testing,
};

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
    let mut pipeline = Pipeline::new(fail_fast);
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
                    }],
                    total: std::time::Duration::ZERO,
                    passed: false,
                };
            }
        };
        if step.gate {
            pipeline = pipeline.gate(&step.name, f);
        } else {
            pipeline = pipeline.step(&step.name, f);
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
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut pipeline = Pipeline::new(fail_fast)
        .gate("self-check", dev_setup::self_check)
        .step("fmt --check", || fmt::run(ctx, true, false));

    if capture_diagnostics {
        let lint_sink: DiagnosticSink = Rc::new(RefCell::new(Vec::new()));
        let lint_sink_clone = lint_sink.clone();
        pipeline = pipeline.step_with_diagnostics("lint", lint_sink, move || {
            let (success, diags) = lint::run_capturing(ctx)?;
            lint_sink_clone.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("clippy found errors"))
            }
        });
    } else {
        pipeline = pipeline.step("lint", || lint::run(ctx, None, false, false));
    }

    pipeline = pipeline.step("compile-tests", || testing::compile::run(ctx));

    if capture_diagnostics {
        let test_sink: DiagnosticSink = Rc::new(RefCell::new(Vec::new()));
        let test_sink_clone = test_sink.clone();
        pipeline = pipeline.step_with_diagnostics("test", test_sink, move || {
            let (success, diags) = testing::run::run_capturing(ctx, offline)?;
            test_sink_clone.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("tests failed"))
            }
        });
    } else {
        pipeline = pipeline.step("test", || {
            testing::run::run(ctx, None, false, false, offline)
        });
    }

    pipeline = pipeline
        .step("check-deps", || check_deps::run(ctx))
        .step("check-protocol-drift", || {
            protocol::drift::run(ctx, false, false, false)
        });

    if let Some(c) = ctx.cov() {
        let crate_name = c.crate_name.clone();
        let threshold = c.threshold();
        pipeline = pipeline.step(&format!("coverage ({crate_name})"), move || {
            testing::coverage::run(ctx, &crate_name, threshold)
        });
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
