use std::path::PathBuf;

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_types::error::TaskitError;
use taskit_types::step::PipelineOutcome;

/// Adapter: runs a Cruxfile via embedded crux-script runtime.
///
/// Currently a stub — full implementation requires the `crux-script` crate.
/// Until then `run_pipeline` returns an error: a runner that executes
/// nothing must never report a passing pipeline.
pub struct EmbeddedCruxRunner {
    cruxfile_path: PathBuf,
}

impl EmbeddedCruxRunner {
    pub fn new(cruxfile_path: PathBuf) -> Self {
        Self { cruxfile_path }
    }
}

impl PipelineRunner for EmbeddedCruxRunner {
    fn run_pipeline(&self, _fail_fast: bool) -> Result<PipelineOutcome, TaskitError> {
        if !self.cruxfile_path.exists() {
            return Err(TaskitError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("cruxfile not found: {}", self.cruxfile_path.display()),
            )));
        }

        Err(TaskitError::other(format!(
            "embedded crux runtime is not implemented yet; cannot run {}",
            self.cruxfile_path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_runner_missing_cruxfile_returns_err() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(false);
        assert!(result.is_err());
    }

    /// The stub must never synthesize a passing outcome: even with an
    /// existing cruxfile, running it reports "not implemented".
    #[test]
    fn embedded_runner_existing_cruxfile_reports_unimplemented() {
        let cruxfile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let runner = EmbeddedCruxRunner::new(cruxfile);
        let err = runner
            .run_pipeline(false)
            .expect_err("stub runner must not report success");
        assert!(
            err.to_string().contains("not implemented"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_cruxfile_error_names_the_path() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let err = runner.run_pipeline(true).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/ci.crux"));
    }
}
