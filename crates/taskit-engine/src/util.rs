use taskit_types::error::TaskitError;
use xshell::Shell;

use crate::{affected, config::WorkspaceConfig};

/// Check if a tool is available by running `<program> [args...]`.
///
/// For most tools this is `tool_exists("cargo-nextest")` → `cargo-nextest --version`.
/// For cargo subcommands whose binary rejects `--version` (e.g. cargo-llvm-cov),
/// use `tool_exists_cmd("cargo", &["llvm-cov", "--version"])` instead.
pub fn tool_exists(name: &str) -> bool {
    tool_exists_cmd(name, &["--version"])
}

pub fn tool_exists_cmd(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Dispatch a per-crate command with single-crate / affected / workspace modes.
///
/// - If `crate_name` is `Some`, run `single_cmd` for that crate only.
/// - If `use_affected` is `true`, detect changed crates via git and run `single_cmd`
///   for each.
/// - Otherwise run `workspace_cmd` once.
///
/// When `continue_on_error` is `true`, per-crate failures are collected and all
/// crates are processed before returning a combined error. Has no effect in
/// workspace mode (a single command covers all crates).
pub fn run_per_crate(
    sh: &Shell,
    ws: &WorkspaceConfig,
    crate_name: Option<&str>,
    use_affected: bool,
    continue_on_error: bool,
    single_cmd: impl Fn(&Shell, &str) -> Result<(), TaskitError>,
    workspace_cmd: impl Fn(&Shell) -> Result<(), TaskitError>,
) -> Result<(), TaskitError> {
    if let Some(name) = crate_name {
        return single_cmd(sh, name);
    }
    if use_affected {
        let crates = affected::detect(sh, ws)?;
        if crates.is_empty() {
            taskit_output::taskit_skip!("No affected crates detected, skipping.");
            return Ok(());
        }
        let mut failed: Vec<String> = Vec::new();
        for crate_dir in &crates {
            let pkg = affected::pkg_name(crate_dir, ws);
            let result = single_cmd(sh, pkg);
            if let Err(e) = result {
                if continue_on_error {
                    taskit_output::taskit_err!("FAILED [{pkg}]: {e}");
                    failed.push(pkg.to_string());
                } else {
                    return Err(e);
                }
            }
        }
        if !failed.is_empty() {
            return Err(TaskitError::other(format!(
                "failed for crate(s): {}",
                failed.join(", ")
            )));
        }
        return Ok(());
    }
    workspace_cmd(sh)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- tool_exists / tool_exists_cmd ---

    #[test]
    fn tool_exists_returns_true_for_cargo() {
        // cargo is always present in the build environment.
        assert!(tool_exists_cmd("cargo", &["--version"]));
    }

    #[test]
    fn tool_exists_returns_false_for_nonexistent_binary() {
        assert!(!tool_exists("__taskit_test_nonexistent_binary_xyz_abc__"));
    }

    #[test]
    fn tool_exists_cmd_returns_false_when_program_exits_nonzero() {
        // "false" is a POSIX utility that always exits 1.
        assert!(!tool_exists_cmd("false", &[]));
    }

    #[test]
    fn tool_exists_cmd_returns_true_for_sh_with_version() {
        assert!(tool_exists_cmd("sh", &["--version"]));
    }

    // --- run_per_crate branching logic ---
    // run_per_crate requires a real xshell::Shell for the affected path (git diff).
    // The single-crate and workspace branches are tested here with a fake shell.

    #[test]
    fn run_per_crate_single_crate_calls_single_cmd() {
        use std::cell::Cell;
        use std::rc::Rc;
        let sh = xshell::Shell::new().expect("shell");
        let ws = crate::config::WorkspaceConfig::default();
        let called = Rc::new(Cell::new(false));
        let called2 = called.clone();
        run_per_crate(
            &sh,
            &ws,
            Some("my-api"),
            false,
            false,
            move |_sh, name| {
                assert_eq!(name, "my-api");
                called2.set(true);
                Ok(())
            },
            |_sh| panic!("workspace_cmd should not be called"),
        )
        .unwrap();
        assert!(called.get());
    }

    #[test]
    fn run_per_crate_workspace_calls_workspace_cmd() {
        use std::cell::Cell;
        use std::rc::Rc;
        let sh = xshell::Shell::new().expect("shell");
        let ws = crate::config::WorkspaceConfig::default();
        let called = Rc::new(Cell::new(false));
        let called2 = called.clone();
        run_per_crate(
            &sh,
            &ws,
            None,
            false,
            false,
            |_sh, _name| panic!("single_cmd should not be called"),
            move |_sh| {
                called2.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(called.get());
    }

    #[test]
    fn run_per_crate_single_crate_propagates_error() {
        let sh = xshell::Shell::new().expect("shell");
        let ws = crate::config::WorkspaceConfig::default();
        let result = run_per_crate(
            &sh,
            &ws,
            Some("my-api"),
            false,
            false,
            |_sh, _name| Err(TaskitError::other("lint failed")),
            |_sh| Ok(()),
        );
        assert!(result.is_err());
    }
}
