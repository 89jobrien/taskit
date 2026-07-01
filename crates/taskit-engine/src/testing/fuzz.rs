use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::runner::xrun;

/// Fuzz target names are cargo target names; anything else (flags, paths,
/// spaces) would be misinterpreted by `cargo fuzz`.
fn is_valid_target(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('-')
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn run(sh: &Shell, target: &str, duration: u64) -> Result<(), TaskitError> {
    if !is_valid_target(target) {
        return Err(TaskitError::other(format!(
            "invalid fuzz target name: {target:?}"
        )));
    }
    let dur = duration.to_string();
    eprintln!("Fuzzing {target} for {dur}s...");
    xrun(cmd!(sh, "cargo fuzz run {target} -- -max_total_time={dur}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typical_target_names() {
        assert!(is_valid_target("fuzz_parse_config"));
        assert!(is_valid_target("fuzz-lockfile-parse"));
    }

    #[test]
    fn rejects_flags_paths_and_empty() {
        assert!(!is_valid_target(""));
        assert!(!is_valid_target("--help"));
        assert!(!is_valid_target("-f"));
        assert!(!is_valid_target("../evil"));
        assert!(!is_valid_target("a b"));
    }
}
