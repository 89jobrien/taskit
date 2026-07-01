use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

/// Accept only `MAJOR.MINOR.PATCH` with numeric components, so the value is
/// safe to hand to the update script (no flags, shell metacharacters, or
/// path traversal).
fn is_valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
}

pub fn run(sh: &Shell, version: &str) -> Result<(), TaskitError> {
    if !is_valid_version(version) {
        return Err(TaskitError::other(format!(
            "invalid version {version:?}: expected MAJOR.MINOR.PATCH (e.g. 2.1.50)"
        )));
    }
    eprintln!("Updating Claude Code version to {version}...");
    xrun(cmd!(sh, "bash scripts/update-claude-version.sh {version}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_semver_triples() {
        assert!(is_valid_version("2.1.50"));
        assert!(is_valid_version("0.0.1"));
        assert!(is_valid_version("10.20.30"));
    }

    #[test]
    fn rejects_flags_and_metacharacters() {
        assert!(!is_valid_version("--help"));
        assert!(!is_valid_version("1.2.3; rm -rf /"));
        assert!(!is_valid_version("$(id)"));
        assert!(!is_valid_version("../../etc/passwd"));
    }

    #[test]
    fn rejects_wrong_shapes() {
        assert!(!is_valid_version(""));
        assert!(!is_valid_version("1.2"));
        assert!(!is_valid_version("1.2.3.4"));
        assert!(!is_valid_version("1.2.x"));
        assert!(!is_valid_version("v1.2.3"));
        assert!(!is_valid_version("1..3"));
    }
}
