use taskit_types::error::TaskitError;
use taskit_types::output_format::OutputFormat;

use crate::{ctx::Ctx, fmt, lint, step::Pipeline, testing};

/// Fast local feedback loop: fmt-check + lint + compile-tests + test.
///
/// All steps operate on affected crates only (git diff vs origin/main) and
/// skip tests that require external network access or credentials.
/// No coverage, schema, or drift checks — those live in `taskit ci`.
///
/// Tool stdout/stderr is suppressed; only progress spinners and the final
/// summary table are shown.  On failure the captured output is included in
/// the error message.
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    ctx.with_silent(|| {
        let outcome = Pipeline::new(false)
            .step("fmt --check (affected)", || fmt::run(ctx, true, true))
            .step("lint (affected)", || lint::run(ctx, None, true, false))
            .step("compile-tests", || testing::compile::run(ctx))
            .step("test (affected, offline)", || {
                testing::run::run(ctx, None, true, false, true)
            })
            .run();
        taskit_output::write_output(OutputFormat::Human, &outcome).map_err(TaskitError::Pipeline)
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
        let _: fn(&crate::ctx::Ctx) -> Result<(), taskit_types::error::TaskitError> = super::run;
    }
}
