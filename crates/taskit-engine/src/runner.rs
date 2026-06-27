use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};

static DRY_RUN: AtomicBool = AtomicBool::new(false);
static SILENT: AtomicBool = AtomicBool::new(false);

pub fn set_dry_run(v: bool) {
    DRY_RUN.store(v, Ordering::Release);
}

pub fn is_dry_run() -> bool {
    DRY_RUN.load(Ordering::Acquire)
}

/// Suppress child-process stdout/stderr for the duration of `f`.
///
/// Used by `cargo xtask quick` so that only the progress spinners are visible.
/// The captured output is attached to the error on failure so nothing is lost.
pub fn with_silent<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let prev = SILENT.swap(true, Ordering::AcqRel);
    let result = f();
    SILENT.store(prev, Ordering::Release);
    result
}

/// Run a shell command, or in dry-run mode print it instead.
///
/// `.quiet()` suppresses xshell's `$ cmd args` echo; our own progress
/// spinners provide the user-facing feedback instead.
///
/// When [`with_silent`] is active, stdout and stderr are captured and
/// discarded on success.  On failure they are attached to the error.
pub fn xrun(cmd: xshell::Cmd<'_>) -> Result<()> {
    if is_dry_run() {
        eprintln!("dry-run: {cmd}");
        return Ok(());
    }
    if SILENT.load(Ordering::Acquire) {
        let label = cmd.to_string();
        let out = cmd.quiet().output().with_context(|| label.clone())?;
        if !out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!(
                "{label} failed (exit {})\n{stdout}{stderr}",
                out.status.code().unwrap_or(-1)
            );
        }
        return Ok(());
    }
    cmd.quiet().run()?;
    Ok(())
}

/// Best-effort run (ignores errors), or in dry-run mode print instead.
#[allow(dead_code)]
pub fn xrun_ok(cmd: xshell::Cmd<'_>) {
    if is_dry_run() {
        eprintln!("dry-run: {cmd}");
    } else {
        let _ = cmd.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Save/restore the global dry_run flag around a closure to isolate tests.
    fn with_dry_run<F: FnOnce()>(v: bool, f: F) {
        let prev = is_dry_run();
        set_dry_run(v);
        f();
        set_dry_run(prev);
    }

    #[test]
    fn set_dry_run_true_makes_is_dry_run_return_true() {
        with_dry_run(true, || assert!(is_dry_run()));
    }

    #[test]
    fn set_dry_run_false_makes_is_dry_run_return_false() {
        with_dry_run(false, || assert!(!is_dry_run()));
    }

    #[test]
    fn dry_run_flag_is_readable_without_panic() {
        // Baseline: the flag is readable regardless of its current state.
        let _ = is_dry_run();
    }

    #[test]
    fn set_dry_run_is_idempotent() {
        with_dry_run(true, || {
            set_dry_run(true);
            assert!(is_dry_run());
        });
    }

    #[test]
    fn dry_run_toggle_round_trips() {
        with_dry_run(false, || {
            set_dry_run(true);
            assert!(is_dry_run());
            set_dry_run(false);
            assert!(!is_dry_run());
        });
    }
}
