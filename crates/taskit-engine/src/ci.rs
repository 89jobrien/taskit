use std::cell::RefCell;
use std::rc::Rc;

use taskit_types::error::TaskitError;
use xshell::Shell;

use crate::{
    check_deps,
    config::{CiConfig, CiStep, CoverageConfig, ProtocolConfig, WorkspaceConfig},
    dev_setup, fmt, lint,
    output::OutputFormat,
    protocol,
    step::{DiagnosticSink, Pipeline, PipelineOutcome},
    testing,
};

/// Options controlling a CI pipeline run.
#[derive(Debug, Clone, Copy)]
pub struct CiOptions {
    pub fail_fast: bool,
    pub include_network: bool,
    pub output_format: OutputFormat,
}

/// Shared context handed to every step factory.
#[derive(Clone, Copy)]
pub(crate) struct StepContext<'a> {
    pub sh: &'a Shell,
    pub ws: &'a WorkspaceConfig,
    pub proto: Option<&'a ProtocolConfig>,
    pub cov: Option<&'a CoverageConfig>,
    pub offline: bool,
}

type StepFn<'a> = Box<dyn FnOnce() -> Result<(), TaskitError> + 'a>;

/// A dispatched step: either a plain closure or one that also collects
/// per-finding diagnostics (used for SARIF output).
enum StepExec<'a> {
    Plain(StepFn<'a>),
    Captured(DiagnosticSink, StepFn<'a>),
}

/// Run the CI pipeline.
///
/// When `ci` contains steps they are dispatched dynamically from the config,
/// allowing workspaces to define their own pipeline in `taskit.toml`.
/// When `ci` is `None` the built-in default steps are used; an explicit
/// `[ci]` with empty steps runs nothing.
pub fn run(
    sh: &Shell,
    ws: &WorkspaceConfig,
    proto: Option<&ProtocolConfig>,
    ci: Option<&CiConfig>,
    cov: Option<&CoverageConfig>,
    opts: CiOptions,
) -> Result<(), TaskitError> {
    let ctx = StepContext {
        sh,
        ws,
        proto,
        cov,
        offline: !opts.include_network,
    };
    let capture = matches!(opts.output_format, OutputFormat::Sarif);
    let outcome = run_pipeline_internal(ctx, ci, opts.fail_fast, capture);
    Ok(crate::output::write_output(opts.output_format, &outcome)?)
}

/// Select the step list (configured or default) and run it.
pub(crate) fn run_pipeline_internal(
    ctx: StepContext<'_>,
    ci: Option<&CiConfig>,
    fail_fast: bool,
    capture: bool,
) -> PipelineOutcome {
    match ci {
        Some(cfg) if !cfg.steps.is_empty() => build_and_run(ctx, &cfg.steps, fail_fast, capture),
        // Explicit [ci] with empty steps = run nothing
        Some(_) => Pipeline::new(fail_fast).run(),
        None => build_and_run(ctx, &default_steps(ctx.cov), fail_fast, capture),
    }
}

/// The built-in default step list, used when no `[[ci.steps]]` are
/// configured. Expressed as `CiStep` data so it flows through the same
/// dispatch as user-configured pipelines.
fn default_steps(cov: Option<&CoverageConfig>) -> Vec<CiStep> {
    let step = |name: &str, cmd: &str, gate: bool| CiStep {
        name: name.to_string(),
        cmd: cmd.to_string(),
        gate,
    };
    let mut steps = vec![
        step("self-check", "self-check", true),
        step("fmt --check", "fmt --check", false),
        step("lint", "lint", false),
        step("compile-tests", "compile-tests", false),
        step("test", "test", false),
        step("check-deps", "check-deps", false),
        step("check-protocol-drift", "check-protocol-drift", false),
    ];
    if let Some(c) = cov {
        steps.push(step(
            &format!("coverage ({})", c.crate_name),
            "coverage",
            false,
        ));
    }
    steps
}

/// Build a pipeline from step configs and run it. A dispatch failure
/// (unknown command) becomes a single-step failure outcome.
fn build_and_run(
    ctx: StepContext<'_>,
    steps: &[CiStep],
    fail_fast: bool,
    capture: bool,
) -> PipelineOutcome {
    let mut pipeline = Pipeline::new(fail_fast);
    for step in steps {
        match dispatch_cmd(&step.cmd, ctx, capture) {
            Ok(StepExec::Plain(f)) if step.gate => pipeline = pipeline.gate(&step.name, f),
            Ok(StepExec::Plain(f)) => pipeline = pipeline.step(&step.name, f),
            Ok(StepExec::Captured(sink, f)) => {
                pipeline = pipeline.step_with_diagnostics(&step.name, sink, f);
            }
            Err(e) => return dispatch_failure(step, e),
        }
    }
    pipeline.run()
}

fn dispatch_failure(step: &CiStep, e: TaskitError) -> PipelineOutcome {
    PipelineOutcome {
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
    }
}

/// Map a `cmd` string to the corresponding built-in step.
///
/// The `cmd` syntax mirrors taskit's CLI subcommands:
/// `"fmt --check"`, `"lint"`, `"test"`, `"coverage"`, `"compile-tests"`,
/// `"check-deps"`, `"check-protocol-drift"`, `"self-check"`, `"health"`.
///
/// When `capture` is set, steps that can report per-finding diagnostics
/// (`lint`, `test`) return a `Captured` exec with an attached sink.
fn dispatch_cmd<'a>(
    cmd: &str,
    ctx: StepContext<'a>,
    capture: bool,
) -> Result<StepExec<'a>, TaskitError> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let sub = *parts.first().unwrap_or(&"");
    let exec = match sub {
        "fmt" => {
            let check = parts.contains(&"--check");
            plain(move || fmt::run(ctx.sh, ctx.ws, check, false))
        }
        "lint" if capture => captured(move |sink| {
            let (success, diags) = lint::run_capturing(ctx.sh, ctx.ws)?;
            sink.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("clippy found errors"))
            }
        }),
        "lint" => plain(move || lint::run(ctx.sh, ctx.ws, None, false, false)),
        "compile-tests" => plain(move || testing::compile::run(ctx.sh)),
        "test" if capture => captured(move |sink| {
            let (success, diags) = testing::run::run_capturing(ctx.sh, ctx.ws, ctx.offline)?;
            sink.borrow_mut().extend(diags);
            if success {
                Ok(())
            } else {
                Err(TaskitError::other("tests failed"))
            }
        }),
        "test" => plain(move || testing::run::run(ctx.sh, ctx.ws, None, false, false, ctx.offline)),
        "coverage" => plain(move || match ctx.cov {
            Some(c) => testing::coverage::run(ctx.sh, &c.crate_name, c.threshold()),
            None => {
                eprintln!("coverage: skipped (no [coverage] in taskit.toml)");
                Ok(())
            }
        }),
        "check-deps" => plain(move || check_deps::run(ctx.sh)),
        "check-protocol-drift" => plain(move || {
            let root = std::env::current_dir()?;
            protocol::drift::run(&root, ctx.proto, false, false, false)
        }),
        "self-check" => plain(dev_setup::self_check),
        "health" => {
            let root = std::env::current_dir()?;
            plain(move || crate::health::run(ctx.sh, &root, false))
        }
        other => {
            return Err(TaskitError::other(format!(
                "unknown ci step command: {other:?}"
            )));
        }
    };
    Ok(exec)
}

fn plain<'a>(f: impl FnOnce() -> Result<(), TaskitError> + 'a) -> StepExec<'a> {
    StepExec::Plain(Box::new(f))
}

fn captured<'a>(f: impl FnOnce(&DiagnosticSink) -> Result<(), TaskitError> + 'a) -> StepExec<'a> {
    let sink: DiagnosticSink = Rc::new(RefCell::new(Vec::new()));
    let sink_in = sink.clone();
    StepExec::Captured(sink, Box::new(move || f(&sink_in)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sh() -> Shell {
        Shell::new().expect("shell")
    }

    fn make_ctx<'a>(sh: &'a Shell, ws: &'a WorkspaceConfig) -> StepContext<'a> {
        StepContext {
            sh,
            ws,
            proto: None,
            cov: None,
            offline: false,
        }
    }

    // --- dispatch_cmd ---

    #[test]
    fn dispatch_cmd_unknown_returns_error() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        match dispatch_cmd("frobnicate", make_ctx(&sh, &ws), false) {
            Err(e) => assert!(
                e.to_string().contains("unknown ci step command"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected Err for unknown command"),
        }
    }

    #[test]
    fn dispatch_cmd_empty_string_returns_error() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        assert!(dispatch_cmd("", make_ctx(&sh, &ws), false).is_err());
    }

    #[test]
    fn dispatch_cmd_known_cmds_return_ok() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
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
                dispatch_cmd(cmd, make_ctx(&sh, &ws), false).is_ok(),
                "dispatch_cmd({cmd:?}) should return Ok"
            );
        }
    }

    #[test]
    fn dispatch_cmd_fmt_check_flag_parsed() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        assert!(dispatch_cmd("fmt --check", make_ctx(&sh, &ws), false).is_ok());
        assert!(dispatch_cmd("fmt", make_ctx(&sh, &ws), false).is_ok());
    }

    #[test]
    fn dispatch_cmd_capture_attaches_sink_for_lint_and_test() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        for cmd in ["lint", "test"] {
            match dispatch_cmd(cmd, make_ctx(&sh, &ws), true) {
                Ok(StepExec::Captured(..)) => {}
                _ => panic!("{cmd} with capture should dispatch as Captured"),
            }
        }
    }

    // --- default_steps ---

    #[test]
    fn default_steps_start_with_self_check_gate() {
        let steps = default_steps(None);
        assert_eq!(steps[0].cmd, "self-check");
        assert!(steps[0].gate);
        assert!(steps.iter().all(|s| !s.cmd.is_empty()));
    }

    #[test]
    fn default_steps_include_coverage_only_when_configured() {
        assert!(!default_steps(None).iter().any(|s| s.cmd == "coverage"));
        let cov = CoverageConfig {
            crate_name: "my-crate".into(),
            threshold: None,
        };
        let steps = default_steps(Some(&cov));
        let last = steps.last().unwrap();
        assert_eq!(last.cmd, "coverage");
        assert_eq!(last.name, "coverage (my-crate)");
    }

    #[test]
    fn default_steps_all_dispatch() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        for s in default_steps(None) {
            assert!(
                dispatch_cmd(&s.cmd, make_ctx(&sh, &ws), false).is_ok(),
                "default step {:?} must dispatch",
                s.cmd
            );
        }
    }

    // --- run with empty steps ---

    #[test]
    fn empty_steps_runs_nothing() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        let cfg = CiConfig {
            steps: vec![],
            cruxfile: None,
        };
        // Empty steps = run nothing (not the default pipeline)
        let outcome = run_pipeline_internal(make_ctx(&sh, &ws), Some(&cfg), false, false);
        assert!(outcome.passed);
        assert!(outcome.results.is_empty());
    }

    #[test]
    fn unknown_step_produces_failure_outcome() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        let cfg = CiConfig {
            steps: vec![CiStep {
                name: "bad".into(),
                cmd: "frobnicate".into(),
                gate: false,
            }],
            cruxfile: None,
        };
        let outcome = run_pipeline_internal(make_ctx(&sh, &ws), Some(&cfg), false, false);
        assert!(!outcome.passed);
        assert_eq!(outcome.results.len(), 1);
        assert!(
            outcome.results[0]
                .error
                .as_deref()
                .unwrap_or("")
                .contains("unknown")
        );
    }
}
