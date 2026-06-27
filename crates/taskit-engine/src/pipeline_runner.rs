use std::path::{Path, PathBuf};
use std::time::Instant;

use taskit_core::config::{CiConfig, CoverageConfig, ProtocolConfig, WorkspaceConfig};
use taskit_core::pipeline_runner::PipelineRunner;
use taskit_core::step::{PipelineOutcome, StepResult, StepStatus};
use xshell::Shell;

/// Adapter: runs the built-in pipeline using taskit's native step engine.
pub struct BuiltinRunner<'a> {
    pub sh: &'a Shell,
    pub ws: &'a WorkspaceConfig,
    pub proto: Option<&'a ProtocolConfig>,
    pub cov: Option<&'a CoverageConfig>,
    pub ci: Option<&'a CiConfig>,
    pub offline: bool,
}

impl PipelineRunner for BuiltinRunner<'_> {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        fail_fast: bool,
    ) -> anyhow::Result<PipelineOutcome> {
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
            _ => crate::ci::run_default_internal(
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
    pub cruxfile_path: PathBuf,
}

impl PipelineRunner for SubprocessCruxRunner {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        _fail_fast: bool,
    ) -> anyhow::Result<PipelineOutcome> {
        if !self.cruxfile_path.exists() {
            anyhow::bail!("cruxfile not found: {}", self.cruxfile_path.display());
        }

        let start = Instant::now();
        let output = std::process::Command::new("crux")
            .arg("run")
            .arg(&self.cruxfile_path)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run crux: {e}"))?;

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
        let sh = Shell::new().unwrap();
        let ws = WorkspaceConfig::default();
        let runner = BuiltinRunner {
            sh: &sh,
            ws: &ws,
            proto: None,
            cov: None,
            ci: None,
            offline: false,
        };
        let _: &dyn PipelineRunner = &runner;
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
        let runner = BuiltinRunner {
            sh: &sh,
            ws: &ws,
            proto: None,
            cov: None,
            ci: Some(&ci),
            offline: false,
        };
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
        let runner = SubprocessCruxRunner {
            cruxfile_path: PathBuf::from("/nonexistent/ci.crux"),
        };
        let result = runner.run_pipeline(Path::new("/nonexistent/ci.crux"), false);
        assert!(result.is_err());
    }

    #[test]
    fn subprocess_runner_implements_trait() {
        let runner = SubprocessCruxRunner {
            cruxfile_path: PathBuf::from("ci.crux"),
        };
        let _: &dyn PipelineRunner = &runner;
    }

    // ── Conformance ─────────────────────────────────────────────────────

    #[test]
    fn subprocess_runner_conformance() {
        let runner = SubprocessCruxRunner {
            cruxfile_path: PathBuf::from("/nonexistent"),
        };
        assert_pipeline_runner_contract(&runner);
    }
}
