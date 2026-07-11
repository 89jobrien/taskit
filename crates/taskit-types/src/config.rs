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
    pub inspect: Option<InspectConfig>,
    pub clean: Option<CleanConfig>,
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
    pub fn lockfile_path(&self) -> &str {
        self.lockfile.as_deref().unwrap_or("taskit-protocol.lock")
    }
}

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
    /// Stop the pipeline on the first failing step.
    pub fail_fast: Option<bool>,
}

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
        match self.threshold {
            Some(threshold) if threshold.is_finite() && threshold > 0.0 => threshold,
            _ => DEFAULT_COVERAGE_THRESHOLD,
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct FlowConfig {
    pub main: Option<String>,
    pub develop: Option<String>,
    pub staging: Option<String>,
    pub release: Option<String>,
}

impl FlowConfig {
    pub fn main_branch(&self) -> &str {
        self.main.as_deref().unwrap_or("main")
    }

    /// Primary development branch; work lands here first.
    pub fn develop_branch(&self) -> &str {
        self.develop.as_deref().unwrap_or("develop")
    }

    pub fn staging_branch(&self) -> &str {
        self.staging.as_deref().unwrap_or("staging")
    }

    pub fn release_branch(&self) -> &str {
        self.release.as_deref().unwrap_or("release")
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ReleaseConfig {
    /// GitHub repo in `owner/name` format (e.g. `89jobrien/taskit`).
    pub github_repo: Option<String>,
    /// Crate publish order (topological). If omitted, workspace members
    /// are published in the order listed in `[workspace] crates`.
    #[serde(default)]
    pub publish_order: Vec<String>,
    /// Skip `cargo doc` generation before publishing.
    pub skip_docs: Option<bool>,
    /// Allow publishing with uncommitted changes.
    pub allow_dirty: Option<bool>,
}

impl ReleaseConfig {
    pub fn github_repo(&self) -> Option<&str> {
        self.github_repo.as_deref()
    }
}

/// Metric thresholds for `taskit inspect`. All fields are optional; absent
/// fields are not checked. CLI flags override values set here.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct InspectConfig {
    /// Maximum allowed clippy warnings before `taskit inspect` fails.
    pub max_clippy_warnings: Option<usize>,
    /// Maximum allowed clippy errors before `taskit inspect` fails.
    pub max_clippy_errors: Option<usize>,
    /// Maximum allowed test failures before `taskit inspect` fails.
    pub max_test_failures: Option<usize>,
    /// Maximum allowed TODO/FIXME markers (unchecked if absent).
    pub max_todo_fixme: Option<usize>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct CleanConfig {
    /// Remove artifacts older than this many days (e.g. `"7d"`).
    /// Uses `cargo sweep` when set; falls back to `cargo clean` otherwise.
    pub older_than: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_flow_config_branch_names_nonempty(
            main in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
            develop in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
            staging in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
            release in proptest::option::of("[a-z][a-z0-9-]{0,20}"),
        ) {
            let cfg = FlowConfig { main, develop, staging, release };
            prop_assert!(!cfg.main_branch().is_empty());
            prop_assert!(!cfg.develop_branch().is_empty());
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

        #[test]
        fn prop_coverage_threshold_positive_finite(
            threshold in proptest::option::of(-100.0f64..=100.0f64),
        ) {
            let cfg = CoverageConfig {
                crate_name: "test".into(),
                threshold,
            };
            let t = cfg.threshold();
            prop_assert!(t > 0.0);
            prop_assert!(t.is_finite());
        }

        #[test]
        fn prop_release_config_publish_order_preserved(
            publish_order in proptest::collection::vec("[a-z][a-z0-9-]{1,20}", 0..=5),
            skip_docs in proptest::option::of(proptest::bool::ANY),
            allow_dirty in proptest::option::of(proptest::bool::ANY),
        ) {
            let cfg = ReleaseConfig {
                github_repo: None,
                publish_order: publish_order.clone(),
                skip_docs,
                allow_dirty,
            };
            prop_assert_eq!(cfg.github_repo(), None);
            prop_assert_eq!(cfg.publish_order, publish_order);
            prop_assert_eq!(cfg.skip_docs, skip_docs);
            prop_assert_eq!(cfg.allow_dirty, allow_dirty);
        }
    }

    #[test]
    fn workspace_offline_skip_expr_returns_none_when_unset() {
        let cfg = WorkspaceConfig::default();
        assert_eq!(cfg.offline_skip_expr(), None);
    }

    #[test]
    fn workspace_offline_skip_expr_returns_configured_value() {
        let cfg = WorkspaceConfig {
            offline_skip: Some("not test(network)".into()),
            ..Default::default()
        };
        assert_eq!(
            cfg.offline_skip_expr().as_deref(),
            Some("not test(network)")
        );
    }

    #[test]
    fn propagation_entry_constructs_source_and_dependents() {
        let entry = PropagationEntry {
            source: "taskit-types".into(),
            dependents: vec!["taskit-engine".into(), "taskit-output".into()],
        };
        assert_eq!(entry.source, "taskit-types");
        assert_eq!(entry.dependents, vec!["taskit-engine", "taskit-output"]);
    }

    #[test]
    fn protocol_lockfile_path_defaults_when_unset() {
        let cfg = ProtocolConfig {
            surfaces: vec![],
            lockfile: None,
        };
        assert_eq!(cfg.lockfile_path(), "taskit-protocol.lock");
    }

    #[test]
    fn protocol_lockfile_path_uses_explicit_value() {
        let cfg = ProtocolConfig {
            surfaces: vec![],
            lockfile: Some("custom.lock".into()),
        };
        assert_eq!(cfg.lockfile_path(), "custom.lock");
    }

    #[test]
    fn surface_entry_deserializes_name_and_path() {
        let entry: SurfaceEntry = toml::from_str(
            r#"
            name = "core-api"
            path = "crates/taskit-core/src/lib.rs"
            "#,
        )
        .expect("surface entry TOML should parse");
        assert_eq!(entry.name, "core-api");
        assert_eq!(entry.path, "crates/taskit-core/src/lib.rs");
    }

    #[test]
    fn ci_step_gate_defaults_to_false() {
        let step: CiStep = toml::from_str(
            r#"
            name = "fmt"
            cmd = "fmt --check"
            "#,
        )
        .unwrap();
        assert_eq!(step.name, "fmt");
        assert_eq!(step.cmd, "fmt --check");
        assert!(!step.gate);
    }

    #[test]
    fn ci_step_parses_explicit_gate() {
        let step: CiStep = toml::from_str(
            r#"
            name = "lint"
            cmd = "lint"
            gate = true
            "#,
        )
        .unwrap();
        assert!(step.gate);
    }

    #[test]
    fn release_config_github_repo_returns_none_when_unset() {
        let cfg = ReleaseConfig::default();
        assert_eq!(cfg.github_repo(), None);
        assert!(cfg.publish_order.is_empty());
    }

    #[test]
    fn release_config_github_repo_returns_configured_repo() {
        let cfg = ReleaseConfig {
            github_repo: Some("89jobrien/taskit".into()),
            publish_order: vec!["taskit-types".into()],
            ..Default::default()
        };
        assert_eq!(cfg.github_repo(), Some("89jobrien/taskit"));
        assert_eq!(cfg.publish_order, vec!["taskit-types"]);
    }

    #[test]
    fn flow_config_default_branch_names_are_exact() {
        let cfg = FlowConfig::default();
        assert_eq!(cfg.main_branch(), "main");
        assert_eq!(cfg.develop_branch(), "develop");
        assert_eq!(cfg.staging_branch(), "staging");
        assert_eq!(cfg.release_branch(), "release");
    }
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

    #[test]
    fn coverage_threshold_default() {
        let cfg = CoverageConfig {
            crate_name: "test".into(),
            threshold: None,
        };
        assert_eq!(cfg.threshold(), 80.0);
    }

    #[test]
    fn coverage_threshold_preserves_positive_finite_value() {
        let cfg = CoverageConfig {
            crate_name: "test".into(),
            threshold: Some(92.5),
        };
        assert_eq!(cfg.threshold(), 92.5);
    }

    #[test]
    fn inspect_config_deserializes_all_fields() {
        let cfg: InspectConfig = toml::from_str(
            r#"
            max_clippy_warnings = 5
            max_clippy_errors   = 2
            max_test_failures   = 0
            max_todo_fixme      = 20
            "#,
        )
        .expect("InspectConfig TOML should parse");
        assert_eq!(cfg.max_clippy_warnings, Some(5));
        assert_eq!(cfg.max_clippy_errors, Some(2));
        assert_eq!(cfg.max_test_failures, Some(0));
        assert_eq!(cfg.max_todo_fixme, Some(20));
    }

    #[test]
    fn inspect_config_all_fields_optional() {
        let cfg: InspectConfig = toml::from_str("").expect("empty InspectConfig should parse");
        assert!(cfg.max_clippy_warnings.is_none());
        assert!(cfg.max_clippy_errors.is_none());
        assert!(cfg.max_test_failures.is_none());
        assert!(cfg.max_todo_fixme.is_none());
    }

    #[test]
    fn clean_config_deserializes_older_than() {
        let cfg: CleanConfig =
            toml::from_str(r#"older_than = "7d""#).expect("CleanConfig TOML should parse");
        assert_eq!(cfg.older_than.as_deref(), Some("7d"));
    }

    #[test]
    fn clean_config_older_than_optional() {
        let cfg: CleanConfig = toml::from_str("").expect("empty CleanConfig should parse");
        assert!(cfg.older_than.is_none());
    }

    #[test]
    fn ci_config_fail_fast_parses() {
        let cfg: CiConfig =
            toml::from_str("fail_fast = true").expect("CiConfig fail_fast should parse");
        assert_eq!(cfg.fail_fast, Some(true));
    }

    #[test]
    fn ci_config_fail_fast_defaults_to_none() {
        let cfg: CiConfig = toml::from_str("").unwrap();
        assert!(cfg.fail_fast.is_none());
    }

    #[test]
    fn release_config_skip_docs_and_allow_dirty_parse() {
        let cfg: ReleaseConfig = toml::from_str("skip_docs = true\nallow_dirty = false").unwrap();
        assert_eq!(cfg.skip_docs, Some(true));
        assert_eq!(cfg.allow_dirty, Some(false));
    }

    #[test]
    fn coverage_threshold_defaults_for_zero_negative_and_non_finite_values() {
        for threshold in [Some(0.0), Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
            let cfg = CoverageConfig {
                crate_name: "test".into(),
                threshold,
            };
            assert_eq!(cfg.threshold(), DEFAULT_COVERAGE_THRESHOLD);
        }
    }
}
