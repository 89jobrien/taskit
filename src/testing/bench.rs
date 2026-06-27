use anyhow::Result;
use xshell::{Shell, cmd};

use crate::runner::xrun;

/// Build the argument list for `cargo bench`, based on optional crate filter and baseline flag.
fn build_bench_args(crate_name: Option<&str>, save_baseline: bool) -> Vec<String> {
    let mut args = vec!["bench".to_string()];
    if let Some(name) = crate_name {
        args.extend(["-p".to_string(), name.to_string()]);
    }
    if save_baseline {
        args.extend([
            "--".to_string(),
            "--save-baseline".to_string(),
            "main".to_string(),
        ]);
    }
    args
}

pub fn run(sh: &Shell, crate_name: Option<&str>, save_baseline: bool) -> Result<()> {
    let args = build_bench_args(crate_name, save_baseline);
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    xrun(cmd!(sh, "cargo {args_ref...}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_bench_args_workspace_no_baseline() {
        let args = build_bench_args(None, false);
        assert_eq!(args, vec!["bench"]);
    }

    #[test]
    fn build_bench_args_single_crate() {
        let args = build_bench_args(Some("maestro-api"), false);
        assert_eq!(args, vec!["bench", "-p", "maestro-api"]);
    }

    #[test]
    fn build_bench_args_workspace_with_baseline() {
        let args = build_bench_args(None, true);
        assert_eq!(args, vec!["bench", "--", "--save-baseline", "main"]);
    }

    #[test]
    fn build_bench_args_single_crate_with_baseline() {
        let args = build_bench_args(Some("maestro-api"), true);
        assert_eq!(
            args,
            vec![
                "bench",
                "-p",
                "maestro-api",
                "--",
                "--save-baseline",
                "main"
            ]
        );
    }

    #[test]
    fn build_bench_args_always_starts_with_bench() {
        for (name, baseline) in [
            (None, false),
            (None, true),
            (Some("foo"), false),
            (Some("foo"), true),
        ] {
            let args = build_bench_args(name, baseline);
            assert_eq!(args[0], "bench");
        }
    }
}
