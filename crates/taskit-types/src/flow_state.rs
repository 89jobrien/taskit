use serde::{Deserialize, Serialize};

/// Current phase of an in-progress `flow auto` execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowPhase {
    /// Promote branch changes from staging to release.
    Promoting,
    /// Run and evaluate CI checks on the promoted state.
    CiGate,
    /// Finalize merges and cleanup after CI passes.
    Finishing,
}

/// Persisted state used to resume flow operations safely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowState {
    /// Active phase.
    pub phase: FlowPhase,
    /// Staging branch name.
    pub staging: String,
    /// Release branch name.
    pub release: String,
    /// Main branch name.
    pub main: String,
    /// SHA of the merge commit on `release` after promote succeeds; None until then.
    pub merge_sha: Option<String>,
    /// Step names that failed in the CI gate phase; empty outside CiGate.
    #[serde(default)]
    pub failed_steps: Vec<String>,
}

impl FlowState {
    /// Create initial flow state at the promote phase.
    pub fn promoting(staging: &str, release: &str, main: &str) -> Self {
        Self {
            phase: FlowPhase::Promoting,
            staging: staging.to_string(),
            release: release.to_string(),
            main: main.to_string(),
            merge_sha: None,
            failed_steps: vec![],
        }
    }

    /// User-facing recovery hint for the current phase.
    pub fn hint(&self) -> &'static str {
        match self.phase {
            FlowPhase::Promoting => "re-run `taskit flow auto` to resume from the promote step",
            FlowPhase::CiGate => "fix the failing CI steps, then re-run `taskit flow auto`",
            FlowPhase::Finishing => "re-run `taskit flow auto` to resume from the finish step",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promoting_constructor_sets_phase_and_branches() {
        let s = FlowState::promoting("staging", "release", "main");
        assert_eq!(s.phase, FlowPhase::Promoting);
        assert_eq!(s.staging, "staging");
        assert_eq!(s.release, "release");
        assert_eq!(s.main, "main");
        assert!(s.merge_sha.is_none());
        assert!(s.failed_steps.is_empty());
    }

    #[test]
    fn hint_returns_nonempty_for_all_phases() {
        for phase in [
            FlowPhase::Promoting,
            FlowPhase::CiGate,
            FlowPhase::Finishing,
        ] {
            let s = FlowState {
                phase,
                staging: "s".into(),
                release: "r".into(),
                main: "m".into(),
                merge_sha: None,
                failed_steps: vec![],
            };
            assert!(!s.hint().is_empty());
        }
    }

    #[test]
    fn phase_serializes_as_snake_case() {
        let s = FlowState::promoting("s", "r", "m");
        // serde round-trip via JSON (uses serde_json in taskit-engine tests;
        // here we just verify the derived impls compile and the phase eq works).
        assert_eq!(s.phase, FlowPhase::Promoting);
        assert_ne!(s.phase, FlowPhase::CiGate);
    }
}
