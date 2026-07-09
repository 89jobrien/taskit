use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

pub fn run(ctx: &Ctx, older_than: Option<&str>) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    if let Some(days) = older_than {
        let days_num = days.strip_suffix('d').unwrap_or(days);
        if days_num.parse::<u64>().is_err() {
            return Err(TaskitError::other(format!(
                "--older-than expects a number of days, optionally suffixed with 'd' (e.g. 7 or 7d), got: {days:?}"
            )));
        }
        taskit_output::taskit_progress!("Sweeping artifacts older than {days_num} days...");
        ctx.run(cmd!(sh, "cargo sweep --time {days_num}"))?;
    } else {
        taskit_output::taskit_progress!("Cleaning target directory...");
        ctx.run(cmd!(sh, "cargo clean"))?;
    }

    prune_artifacts()?;

    Ok(())
}

/// Remove taskit-generated artifacts outside of target/.
fn prune_artifacts() -> Result<(), TaskitError> {
    let artifacts = [".taskit-cache", "target/taskit-results.xml"];
    for path in artifacts {
        let p = std::path::Path::new(path);
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
            taskit_output::taskit_ok!("removed {path}/");
        } else if p.is_file() {
            std::fs::remove_file(p)?;
            taskit_output::taskit_ok!("removed {path}");
        }
    }
    Ok(())
}

#[cfg(test)]
fn check_parse(s: &str) -> bool {
    let days_num = s.strip_suffix('d').unwrap_or(s);
    days_num.parse::<u64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_than_accepts_plain_number() {
        assert!(check_parse("7"));
    }

    #[test]
    fn older_than_accepts_days_suffix() {
        assert!(check_parse("7d"));
    }

    #[test]
    fn older_than_rejects_non_numeric_suffix() {
        assert!(!check_parse("7h"));
        assert!(!check_parse("abc"));
    }

    #[test]
    fn older_than_rejects_multiple_trailing_d() {
        assert!(!check_parse("7dd"));
        assert!(!check_parse("7ddd"));
    }

    #[test]
    fn older_than_accepts_zero() {
        assert!(check_parse("0"));
        assert!(check_parse("0d"));
    }

    #[test]
    fn older_than_accepts_large_number() {
        assert!(check_parse("365"));
        assert!(check_parse("365d"));
    }

    #[test]
    fn older_than_rejects_negative() {
        // Negative numbers are not valid unsigned days.
        assert!(!check_parse("-7"));
        assert!(!check_parse("-7d"));
    }

    #[test]
    fn older_than_rejects_float() {
        assert!(!check_parse("1.5"));
        assert!(!check_parse("1.5d"));
    }

    #[test]
    fn older_than_rejects_empty_string() {
        assert!(!check_parse(""));
    }

    #[test]
    fn older_than_rejects_uppercase_d_suffix() {
        // Only lowercase 'd' is stripped; uppercase is not.
        assert!(!check_parse("7D"));
    }
}
