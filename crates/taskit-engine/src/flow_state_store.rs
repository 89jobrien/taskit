use std::path::Path;

use taskit_types::error::TaskitError;
use taskit_types::flow_state::FlowState;

const STATE_FILE: &str = ".taskit-state.json";

/// Read `.taskit-state.json` from the workspace root.
/// Returns `None` if the file is absent or cannot be parsed.
pub fn load(root: &Path) -> Option<FlowState> {
    let path = root.join(STATE_FILE);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write `.taskit-state.json` atomically (write to `.tmp`, then rename).
pub fn save(root: &Path, state: &FlowState) -> Result<(), TaskitError> {
    let path = root.join(STATE_FILE);
    let tmp = root.join(".taskit-state.json.tmp");
    let json =
        serde_json::to_string_pretty(state).map_err(|e| TaskitError::other(e.to_string()))?;
    std::fs::write(&tmp, json).map_err(|e| TaskitError::other(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| TaskitError::other(e.to_string()))?;
    Ok(())
}

/// Delete `.taskit-state.json`; no-op if absent.
pub fn clear(root: &Path) -> Result<(), TaskitError> {
    let path = root.join(STATE_FILE);
    match std::fs::remove_file(&path) {
        Ok(()) | Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::flow_state::{FlowPhase, FlowState};

    fn tmp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn load_returns_none_when_absent() {
        let dir = tmp_dir();
        assert!(load(dir.path()).is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tmp_dir();
        let state = FlowState::promoting("staging", "release", "main");
        save(dir.path(), &state).expect("save");
        let loaded = load(dir.path()).expect("load");
        assert_eq!(loaded.phase, FlowPhase::Promoting);
        assert_eq!(loaded.staging, "staging");
        assert_eq!(loaded.main, "main");
    }

    #[test]
    fn clear_removes_state_file() {
        let dir = tmp_dir();
        let state = FlowState::promoting("s", "r", "m");
        save(dir.path(), &state).expect("save");
        assert!(dir.path().join(".taskit-state.json").exists());
        clear(dir.path()).expect("clear");
        assert!(!dir.path().join(".taskit-state.json").exists());
    }

    #[test]
    fn clear_is_noop_when_absent() {
        let dir = tmp_dir();
        assert!(clear(dir.path()).is_ok());
    }

    #[test]
    fn save_overwrites_existing_state() {
        let dir = tmp_dir();
        let s1 = FlowState::promoting("s", "r", "m");
        save(dir.path(), &s1).expect("first save");
        let mut s2 = s1.clone();
        s2.phase = FlowPhase::CiGate;
        s2.merge_sha = Some("deadbeef".into());
        save(dir.path(), &s2).expect("second save");
        let loaded = load(dir.path()).expect("load");
        assert_eq!(loaded.phase, FlowPhase::CiGate);
        assert_eq!(loaded.merge_sha.as_deref(), Some("deadbeef"));
    }
}
