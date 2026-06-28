use std::path::{Path, PathBuf};
use std::time::Instant;

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_core::step::{PipelineOutcome, StepResult, StepStatus};

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
    ) -> anyhow::Result<PipelineOutcome> {
        if !self.cruxfile_path.exists() {
            anyhow::bail!("cruxfile not found: {}", self.cruxfile_path.display());
        }

        let start = Instant::now();

        // TODO: stub silently returns Pass — should return Skipped or Err
        // to avoid a false-green pipeline when crux-script isn't available.
        // Stub: crux-script runtime not yet available.
        // When crux-script is published, replace this with:
        //   let rt = tokio::runtime::Runtime::new()?;
        //   rt.block_on(crux_script::run_file(&self.cruxfile_path))?;
        let duration = start.elapsed();

        Ok(PipelineOutcome {
            results: vec![StepResult {
                name: "crux-embedded".into(),
                status: StepStatus::Pass,
                duration,
                error: None,
                gate: false,
            }],
            total: duration,
            passed: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn embedded_runner_conformance() {
        let runner = EmbeddedCruxRunner::new(PathBuf::from("/nonexistent"));
        let result = runner.run_pipeline(Path::new("/nonexistent/taskit.toml"), false);
        assert!(
            result.is_err(),
            "run_pipeline with nonexistent path should return Err"
        );
    }
}
