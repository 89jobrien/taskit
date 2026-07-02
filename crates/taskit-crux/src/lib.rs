use std::path::{Path, PathBuf};
use std::time::Instant;

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_types::error::TaskitError;
use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

/// Adapter: runs a Cruxfile via embedded crux-script runtime.
///
/// Currently a stub — full implementation requires the `crux-script` crate.
pub struct EmbeddedCruxRunner {
    cruxfile_path: PathBuf,
}

impl EmbeddedCruxRunner {
    pub fn new(cruxfile_path: PathBuf) -> Self {
        Self { cruxfile_path }
    }
}

impl PipelineRunner for EmbeddedCruxRunner {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        _fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        if !self.cruxfile_path.exists() {
            return Err(TaskitError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("cruxfile not found: {}", self.cruxfile_path.display()),
            )));
        }

        let start = Instant::now();

        // Stub: crux-script runtime not yet available.
        let duration = start.elapsed();

        Ok(PipelineOutcome {
            results: vec![StepResult {
                name: "crux-embedded".into(),
                status: StepStatus::Pass,
                duration,
                error: None,
                gate: false,
                diagnostics: vec![],
            }],
            total: duration,
            passed: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_core::conformance::{
        assert_duration_invariants, assert_nonexistent_path_returns_err,
        assert_step_names_nonempty, assert_success_outcome_invariants,
    };

    // ── EmbeddedCruxRunner ───────────────────────────────────────────────

    #[test]
    fn embedded_runner_implements_trait() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("taskit.toml"), false);
        assert!(result.is_err(), "missing cruxfile should return Err");
    }

    #[test]
    fn embedded_runner_missing_cruxfile_returns_err() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("taskit.toml"), false);
        assert!(result.is_err());
    }

    // ── Conformance ──────────────────────────────────────────────────────

    /// Invariant 1: nonexistent cruxfile path returns Err.
    #[test]
    fn embedded_runner_conformance_nonexistent_returns_err() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent"));
        assert_nonexistent_path_returns_err(&runner);
    }

    /// Invariants 2, 4, 5: a passing EmbeddedCruxRunner outcome (requires an
    /// existing path — use a real file that is guaranteed to exist on the host).
    #[test]
    fn embedded_runner_conformance_success_outcome() {
        // Use the Cargo.toml of this crate as a stand-in "cruxfile" — the stub
        // runner only checks `path.exists()` before returning a synthetic Pass.
        let cruxfile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let runner = EmbeddedCruxRunner::new(cruxfile);
        let outcome = runner
            .run_pipeline(Path::new("taskit.toml"), false)
            .expect("runner with existing path must return Ok");
        assert_success_outcome_invariants(&outcome);
        assert_duration_invariants(&outcome);
        assert_step_names_nonempty(&outcome);
    }
}
