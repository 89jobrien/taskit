use serde::Deserialize;
use std::path::PathBuf;

pub const DEFAULT_COVERAGE_THRESHOLD: f64 = 80.0;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    pub protocol: Option<ProtocolConfig>,
    pub ci: Option<CiConfig>,
    pub coverage: Option<CoverageConfig>,
    pub flow: Option<FlowConfig>,
    pub release: Option<ReleaseConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WorkspaceConfig {
    pub root: Option<PathBuf>,
    #[serde(default)]
    pub crates: Vec<CrateEntry>,
    #[serde(default)]
    pub propagation: Vec<PropagationEntry>,
    pub offline_skip: Option<String>,
}

impl WorkspaceConfig {
    // TODO(test): unit test for offline_skip_expr() with Some and None values
    pub fn offline_skip_expr(&self) -> Option<String> {
        self.offline_skip.clone()
    }
}

#[derive(Debug, Deserialize)]
pub struct CrateEntry {
    pub dir: String,
    pub pkg: Option<String>,
}

impl CrateEntry {
    pub fn pkg_name(&self) -> &str {
        self.pkg.as_deref().unwrap_or(&self.dir)
    }
}

// TODO(test): unit test for PropagationEntry construction
#[derive(Debug, Clone, Deserialize)]
pub struct PropagationEntry {
    pub source: String,
    pub dependents: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolConfig {
    #[serde(default)]
    pub surfaces: Vec<SurfaceEntry>,
    pub lockfile: Option<String>,
}

impl ProtocolConfig {
    // TODO(test): unit test for lockfile_path() default and explicit values
    pub fn lockfile_path(&self) -> &str {
        self.lockfile.as_deref().unwrap_or("taskit-protocol.lock")
    }
}

// TODO(test): unit test for SurfaceEntry construction
#[derive(Debug, Deserialize)]
pub struct SurfaceEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct CiConfig {
    #[serde(default)]
    pub steps: Vec<CiStep>,
    #[serde(default)]
    pub cruxfile: Option<String>,
}

// TODO(test): unit test for CiStep (gate default, name/cmd)
#[derive(Debug, Deserialize)]
pub struct CiStep {
    pub name: String,
    pub cmd: String,
    #[serde(default)]
    pub gate: bool,
}

#[derive(Debug, Deserialize)]
pub struct CoverageConfig {
    pub crate_name: String,
    pub threshold: Option<f64>,
}

impl CoverageConfig {
    pub fn threshold(&self) -> f64 {
        self.threshold.unwrap_or(DEFAULT_COVERAGE_THRESHOLD)
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct FlowConfig {
    pub main: Option<String>,
    pub staging: Option<String>,
    pub release: Option<String>,
}

impl FlowConfig {
    pub fn main_branch(&self) -> &str {
        self.main.as_deref().unwrap_or("main")
    }

    pub fn staging_branch(&self) -> &str {
        self.staging.as_deref().unwrap_or("staging")
    }

    pub fn release_branch(&self) -> &str {
        self.release.as_deref().unwrap_or("release")
    }
}

// TODO(test): unit + property tests for ReleaseConfig (github_repo getter, empty vs populated)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ReleaseConfig {
    /// GitHub repo in `owner/name` format (e.g. `89jobrien/taskit`).
    pub github_repo: Option<String>,
    /// Crate publish order (topological). If omitted, workspace members
    /// are published in the order listed in `[workspace] crates`.
    #[serde(default)]
    pub publish_order: Vec<String>,
}

impl ReleaseConfig {
    pub fn github_repo(&self) -> Option<&str> {
        self.github_repo.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_flow_config_branch_names_nonempty(
            main in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
            staging in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
            release in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
        ) {
            let cfg = FlowConfig { main, staging, release };
            prop_assert!(!cfg.main_branch().is_empty());
            prop_assert!(!cfg.staging_branch().is_empty());
            prop_assert!(!cfg.release_branch().is_empty());
        }

        #[test]
        fn prop_crate_entry_pkg_name_nonempty(
            dir in "[a-z][a-z0-9-]{1,20}",
            pkg in proptest::option::of("[a-z][a-z0-9-]{1,20}"),
        ) {
            let entry = CrateEntry { dir, pkg };
            prop_assert!(!entry.pkg_name().is_empty());
        }

        // TODO(test): fix latent bug — range includes Some(0.0) but assertion requires t > 0.0
        #[test]
        fn prop_coverage_threshold_positive_finite(
            threshold in proptest::option::of(0.0f64..=100.0f64),
        ) {
            let cfg = CoverageConfig {
                crate_name: "test".into(),
                threshold,
            };
            let t = cfg.threshold();
            prop_assert!(t > 0.0);
            prop_assert!(t.is_finite());
        }
    }

    // TODO(test): unit test asserting exact default branch names ("main", "staging", "release") for None case
    #[test]
    fn ci_config_cruxfile_defaults_to_none() {
        let cfg: CiConfig = toml::from_str("").unwrap();
        assert!(cfg.cruxfile.is_none());
        assert!(cfg.steps.is_empty());
    }

    #[test]
    fn ci_config_cruxfile_parses() {
        let cfg: CiConfig = toml::from_str(r#"cruxfile = "ci.crux""#).unwrap();
        assert_eq!(cfg.cruxfile.as_deref(), Some("ci.crux"));
    }

    // TODO(test): add conformance suite for TaskitResultExt trait contract
    // TODO(test): add integration tests using real taskit.toml fixture files (tests/ dir)
    #[test]
    fn coverage_threshold_default() {
        let cfg = CoverageConfig {
            crate_name: "test".into(),
            threshold: None,
        };
        assert_eq!(cfg.threshold(), 80.0);
    }
}
