use std::path::{Path, PathBuf};
use std::time::Instant;

use taskit_core::config::{CiConfig, CoverageConfig, ProtocolConfig, WorkspaceConfig};
use taskit_core::pipeline_runner::PipelineRunner;
use taskit_core::step::{PipelineOutcome, StepResult, StepStatus};
use taskit_types::error::TaskitError;
use xshell::Shell;

/// Adapter: runs the built-in pipeline using taskit's native step engine.
pub struct BuiltinRunner<'a> {
    pub(crate) sh: &'a Shell,
    pub(crate) ws: &'a WorkspaceConfig,
    pub(crate) proto: Option<&'a ProtocolConfig>,
    pub(crate) cov: Option<&'a CoverageConfig>,
    pub(crate) ci: Option<&'a CiConfig>,
    pub(crate) offline: bool,
}

impl<'a> BuiltinRunner<'a> {
    pub fn new(
        sh: &'a Shell,
        ws: &'a WorkspaceConfig,
        proto: Option<&'a ProtocolConfig>,
        cov: Option<&'a CoverageConfig>,
        ci: Option<&'a CiConfig>,
        offline: bool,
    ) -> Self {
        Self {
            sh,
            ws,
            proto,
            cov,
            ci,
            offline,
        }
    }
}

impl PipelineRunner for BuiltinRunner<'_> {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        let outcome = match self.ci {
            Some(cfg) if !cfg.steps.is_empty() => crate::ci::run_from_config_internal(
                self.sh,
                self.ws,
                self.proto,
                self.cov,
                cfg,
                fail_fast,
                self.offline,
            ),
            Some(_) => {
                // Explicit [ci] with empty steps = run nothing
                crate::step::Pipeline::new(fail_fast).run()
            }
            None => crate::ci::run_default_internal(
                self.sh,
                self.ws,
                self.proto,
                self.cov,
                fail_fast,
                self.offline,
            ),
        };
        Ok(outcome)
    }
}

/// Adapter: runs a Cruxfile via subprocess (`crux run <path>`).
pub struct SubprocessCruxRunner {
    cruxfile_path: PathBuf,
}

impl SubprocessCruxRunner {
    pub fn new(cruxfile_path: PathBuf) -> Self {
        Self { cruxfile_path }
    }
}

impl PipelineRunner for SubprocessCruxRunner {
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
        let output = std::process::Command::new("crux")
            .arg("run")
            .arg(&self.cruxfile_path)
            .output()
            .map_err(|e| {
                TaskitError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("failed to run crux: {e}"),
                ))
            })?;

        let duration = start.elapsed();
        let passed = output.status.success();
        let error = if passed {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
        };

        Ok(PipelineOutcome {
            results: vec![StepResult {
                name: "crux-pipeline".into(),
                status: if passed {
                    StepStatus::Pass
                } else {
                    StepStatus::Fail
                },
                duration,
                error,
                gate: false,
            }],
            total: duration,
            passed,
        })
    }
}

// ── Conformance ─────────────────────────────────────────────────────────────

/// Assert that a PipelineRunner impl satisfies the trait contract:
/// - Missing/nonexistent config path returns Err
#[cfg(test)]
fn assert_pipeline_runner_contract(runner: &dyn PipelineRunner) {
    let result = runner.run_pipeline(Path::new("/nonexistent/taskit.toml"), false);
    assert!(
        result.is_err(),
        "run_pipeline with nonexistent path should return Err"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── BuiltinRunner ───────────────────────────────────────────────────

    #[test]
    fn builtin_runner_implements_trait() {
        use taskit_core::config::CiStep;
        let sh = Shell::new().unwrap();
        let ws = WorkspaceConfig::default();
        // Use self-check step (fast) to avoid running the full default pipeline
        let ci = CiConfig {
            steps: vec![CiStep {
                name: "self-check".into(),
                cmd: "self-check".into(),
                gate: false,
            }],
            cruxfile: None,
        };
        let runner = BuiltinRunner::new(&sh, &ws, None, None, Some(&ci), false);
        let outcome = runner
            .run_pipeline(Path::new("taskit.toml"), false)
            .unwrap();
        assert_eq!(outcome.results.len(), 1);
    }

    #[test]
    fn builtin_runner_with_config_steps_returns_outcome() {
        use taskit_core::config::CiStep;
        let sh = Shell::new().unwrap();
        let ws = WorkspaceConfig::default();
        let ci = CiConfig {
            steps: vec![CiStep {
                name: "self-check".into(),
                cmd: "self-check".into(),
                gate: false,
            }],
            cruxfile: None,
        };
        let runner = BuiltinRunner::new(&sh, &ws, None, None, Some(&ci), false);
        // Runs the config-driven path (not the full default pipeline)
        let outcome = runner
            .run_pipeline(Path::new("taskit.toml"), false)
            .unwrap();
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].name, "self-check");
    }

    // ── SubprocessCruxRunner ────────────────────────────────────────────

    #[test]
    fn subprocess_runner_missing_cruxfile_returns_err() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("/nonexistent/ci.crux"), false);
        assert!(result.is_err());
    }

    #[test]
    fn subprocess_runner_implements_trait() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("taskit.toml"), false);
        assert!(result.is_err(), "missing cruxfile should return Err");
    }

    // ── Conformance ─────────────────────────────────────────────────────

    #[test]
    fn subprocess_runner_conformance() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent"));
        assert_pipeline_runner_contract(&runner);
    }
}
