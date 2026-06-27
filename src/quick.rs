use anyhow::Result;
use xshell::Shell;

use crate::{config::WorkspaceConfig, fmt, lint, runner::with_silent, step::Pipeline, testing};

/// Fast local feedback loop: fmt-check + lint + compile-tests + test.
///
/// All steps operate on affected crates only (git diff vs origin/main) and
/// skip tests that require external network access or credentials.
/// No coverage, schema, or drift checks — those live in `cargo xtask ci`.
///
/// Tool stdout/stderr is suppressed; only progress spinners and the final
/// summary table are shown.  On failure the captured output is included in
/// the error message.
pub fn run(sh: &Shell, ws: &WorkspaceConfig) -> Result<()> {
    with_silent(|| {
        let outcome = Pipeline::new(false)
            .step("fmt --check (affected)", || fmt::run(sh, ws, true, true))
            .step("lint (affected)", || lint::run(sh, ws, None, true, false))
            .step("compile-tests", || testing::compile::run(sh))
            .step("test (affected, offline)", || {
                testing::run::run(sh, ws, None, true, false, true)
            })
            .run();
        crate::step::print_summary(&outcome);
        if outcome.passed {
            Ok(())
        } else {
            anyhow::bail!("quick checks failed")
        }
    })
}

#[cfg(test)]
mod tests {
    // quick::run delegates entirely to Pipeline + existing modules;
    // behaviour is covered by their own unit tests.
    // Smoke-test: the module compiles and the public symbol exists.
    #[test]
    fn quick_run_is_exported() {
        // If this compiles, the public API is intact.
        let _: fn(&xshell::Shell, &crate::config::WorkspaceConfig) -> anyhow::Result<()> =
            super::run;
    }
}
