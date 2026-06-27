use anyhow::{Result, bail};
use xshell::Shell;

use crate::{
    DEFAULT_COVERAGE_THRESHOLD, check_deps,
    config::{CiConfig, ProtocolConfig, WorkspaceConfig},
    dev_setup, fmt, lint, protocol,
    step::Pipeline,
    testing,
};

/// Run the CI pipeline.
///
/// When `ci` contains steps they are dispatched dynamically from the config,
/// allowing workspaces to define their own pipeline in `taskit.toml`.
/// When `ci` is `None` or empty the built-in default pipeline is used.
pub fn run(
    sh: &Shell,
    ws: &WorkspaceConfig,
    proto: Option<&ProtocolConfig>,
    ci: Option<&CiConfig>,
    fail_fast: bool,
    include_network: bool,
) -> Result<()> {
    let offline = !include_network;
    match ci {
        Some(cfg) if !cfg.steps.is_empty() => {
            run_from_config(sh, ws, proto, cfg, fail_fast, offline)
        }
        _ => run_default(sh, ws, proto, fail_fast, offline),
    }
}

/// Build and run a pipeline from `[[ci.steps]]` in `taskit.toml`.
fn run_from_config(
    sh: &Shell,
    ws: &WorkspaceConfig,
    proto: Option<&ProtocolConfig>,
    cfg: &CiConfig,
    fail_fast: bool,
    offline: bool,
) -> Result<()> {
    let mut pipeline = Pipeline::new(fail_fast);
    for step in &cfg.steps {
        let f = dispatch_cmd(&step.cmd, sh, ws, proto, offline)?;
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
/// `"fmt --check"`, `"lint"`, `"test"`, `"coverage"`, `"schema --check"`,
/// `"compile-tests"`, `"check-deps"`, `"check-protocol-drift"`, `"self-check"`.
fn dispatch_cmd<'a>(
    cmd: &str,
    sh: &'a Shell,
    ws: &'a WorkspaceConfig,
    proto: Option<&'a ProtocolConfig>,
    offline: bool,
) -> Result<Box<dyn FnOnce() -> Result<()> + 'a>> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let sub = *parts.first().unwrap_or(&"");
    let f: Box<dyn FnOnce() -> Result<()> + 'a> = match sub {
        "fmt" => {
            let check = parts.contains(&"--check");
            Box::new(move || fmt::run(sh, ws, check, false))
        }
        "lint" => Box::new(move || lint::run(sh, ws, None, false, false)),
        "compile-tests" => Box::new(move || testing::compile::run(sh)),
        "test" => Box::new(move || testing::run::run(sh, ws, None, false, false, offline)),
        "coverage" => {
            Box::new(move || testing::coverage::run(sh, "maestro-api", DEFAULT_COVERAGE_THRESHOLD))
        }
        "check-deps" => Box::new(move || check_deps::run(sh)),
        "check-protocol-drift" => Box::new(move || {
            let root = std::env::current_dir()?;
            protocol::drift::run(&root, proto, false, false, false)
        }),
        "self-check" => Box::new(dev_setup::self_check),
        other => bail!("unknown ci step command: {other:?}"),
    };
    Ok(f)
}

/// The built-in default pipeline, used when no `[[ci.steps]]` are configured.
fn run_default(
    sh: &Shell,
    ws: &WorkspaceConfig,
    proto: Option<&ProtocolConfig>,
    fail_fast: bool,
    offline: bool,
) -> Result<()> {
    Pipeline::new(fail_fast)
        .gate("self-check", dev_setup::self_check)
        .step("fmt --check", || fmt::run(sh, ws, true, false))
        .step("lint", || lint::run(sh, ws, None, false, false))
        .step("compile-tests", || testing::compile::run(sh))
        .step("test", || {
            testing::run::run(sh, ws, None, false, false, offline)
        })
        .step("coverage (maestro-api)", || {
            testing::coverage::run(sh, "maestro-api", DEFAULT_COVERAGE_THRESHOLD)
        })
        .step("check-deps", || check_deps::run(sh))
        .step("check-protocol-drift", || {
            let root = std::env::current_dir()?;
            protocol::drift::run(&root, proto, false, false, false)
        })
        .step("check-protocol-sites", || {
            protocol::sites::run(
                std::path::Path::new("maestro-common/src/session.rs"),
                "CreateSessionRequest {",
                4,
                false,
            )
        })
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sh() -> Shell {
        Shell::new().expect("shell")
    }

    // --- dispatch_cmd ---

    #[test]
    fn dispatch_cmd_unknown_returns_error() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        match dispatch_cmd("frobnicate", &sh, &ws, None, false) {
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
        assert!(dispatch_cmd("", &sh, &ws, None, false).is_err());
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
        ];
        for cmd in known {
            assert!(
                dispatch_cmd(cmd, &sh, &ws, None, false).is_ok(),
                "dispatch_cmd({cmd:?}) should return Ok"
            );
        }
    }

    #[test]
    fn dispatch_cmd_fmt_check_flag_parsed() {
        // Verify the flag is parsed without error (closure not called here).
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        assert!(dispatch_cmd("fmt --check", &sh, &ws, None, false).is_ok());
        assert!(dispatch_cmd("fmt", &sh, &ws, None, false).is_ok());
    }

    // --- run_from_config with empty steps ---

    #[test]
    fn run_from_config_empty_steps_passes() {
        let sh = make_sh();
        let ws = WorkspaceConfig::default();
        let cfg = CiConfig { steps: vec![] };
        // An empty pipeline has no steps and passes trivially.
        let result = run_from_config(&sh, &ws, None, &cfg, false, false);
        assert!(result.is_ok());
    }
}
