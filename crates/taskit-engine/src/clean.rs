use anyhow::{Result, bail};
use xshell::{Shell, cmd};

use crate::runner::xrun;

pub fn run(sh: &Shell, older_than: Option<&str>) -> Result<()> {
    if let Some(days) = older_than {
        let days_num = days.strip_suffix('d').unwrap_or(days);
        if days_num.parse::<u64>().is_err() {
            bail!(
                "--older-than expects a number of days, optionally suffixed with 'd' (e.g. 7 or 7d), got: {days:?}"
            );
        }
        eprintln!("Sweeping artifacts older than {days_num} days...");
        xrun(cmd!(sh, "cargo sweep --time {days_num}"))?;
    } else {
        eprintln!("Cleaning target directory...");
        xrun(cmd!(sh, "cargo clean"))?;
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
