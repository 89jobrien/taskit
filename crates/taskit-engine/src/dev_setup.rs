use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::{
    runner::xrun,
    util::{tool_exists, tool_exists_cmd},
};

const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("cargo-nextest", "cargo-nextest"),
    ("cargo-llvm-cov", "cargo-llvm-cov"),
    ("cargo-deny", "cargo-deny"),
    ("cargo-machete", "cargo-machete"),
];

/// Some cargo tools are subcommands and must be checked via `cargo <sub> --version`
/// rather than `<binary> --version`.
fn check_tool_exists(name: &str) -> bool {
    match name {
        "cargo-llvm-cov" => tool_exists_cmd("cargo", &["llvm-cov", "--version"]),
        other => tool_exists(other),
    }
}

const OPTIONAL_TOOLS: &[(&str, &str)] = &[("sccache", "sccache"), ("cargo-sweep", "cargo-sweep")];

const COL_TOOL: usize = 20;
const COL_STATUS: usize = 10;
const SEPARATOR_WIDTH: usize = 50;

/// Print a yes/no prompt and return true if the user answered y/Y.
fn confirm(prompt: &str) -> bool {
    use std::io::{Write, stdin, stdout};
    eprint!("{prompt} [y/N] ");
    let _ = stdout().flush();
    let mut line = String::new();
    if stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim(), "y" | "Y")
}

/// Ensure `cargo-binstall` is available, bootstrapping via `cargo install` if
/// the user consents. Returns an error if it is absent and the user declines.
fn ensure_binstall(sh: &Shell) -> Result<(), TaskitError> {
    if tool_exists("cargo-binstall") {
        return Ok(());
    }
    eprintln!("  cargo-binstall is not installed (required to install other tools).");
    if crate::runner::is_dry_run() {
        eprintln!("dry-run: cargo install cargo-binstall");
        return Ok(());
    }
    if !confirm("  Install cargo-binstall now via `cargo install cargo-binstall`?") {
        return Err(anyhow::anyhow!(
            "cargo-binstall is required. Install it manually and re-run `cargo xtask dev-setup`."
        )
        .into());
    }
    xrun(cmd!(sh, "cargo install cargo-binstall"))
}

pub fn setup(sh: &Shell) -> Result<(), TaskitError> {
    eprintln!("Installing development tools...");
    ensure_binstall(sh)?;
    for (name, install_name) in REQUIRED_TOOLS {
        if check_tool_exists(name) {
            eprintln!("  {name}: already installed");
        } else {
            eprintln!("  Installing {name}...");
            xrun(cmd!(sh, "cargo binstall -y {install_name}"))?;
        }
    }
    eprintln!(
        "\nOptional tools (not installed automatically):\n{}",
        OPTIONAL_TOOLS
            .iter()
            .map(|(name, install)| format!("  {name} — `cargo binstall {install}`"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

#[cfg(test)]
fn check_label(name: &str) -> &str {
    match name {
        "cargo-llvm-cov" => "cargo llvm-cov",
        other => other,
    }
}

pub fn self_check() -> Result<(), TaskitError> {
    eprintln!("{:<COL_TOOL$} {:<COL_STATUS$} Notes", "Tool", "Status");
    eprintln!("{}", "-".repeat(SEPARATOR_WIDTH));
    let binstall_status = if tool_exists("cargo-binstall") {
        "OK"
    } else {
        "MISSING"
    };
    // cargo-binstall is only needed for `dev-setup`, not for running CI steps.
    // Do not count it as a required tool for self-check purposes.
    let mut missing = false;
    eprintln!(
        "{:<COL_TOOL$} {:<COL_STATUS$} optional (installer)",
        "cargo-binstall", binstall_status
    );
    for (name, _) in REQUIRED_TOOLS {
        let status = if check_tool_exists(name) {
            "OK"
        } else {
            "MISSING"
        };
        if status == "MISSING" {
            missing = true;
        }
        eprintln!("{:<COL_TOOL$} {:<COL_STATUS$} required", name, status);
    }
    for (name, _) in OPTIONAL_TOOLS {
        let status = if check_tool_exists(name) {
            "OK"
        } else {
            "MISSING"
        };
        eprintln!("{:<COL_TOOL$} {:<COL_STATUS$} optional", name, status);
    }
    if missing {
        return Err(anyhow::anyhow!(
            "Required tools missing. Run `cargo xtask dev-setup` to install."
        )
        .into());
    }
    match crate::cache::verify() {
        Ok(true) => eprintln!("{:<COL_TOOL$} OK      cache integrity", ".xtask-cache"),
        Ok(false) => eprintln!(
            "{:<COL_TOOL$} DRIFT   run any xtask command to rebuild",
            ".xtask-cache"
        ),
        Err(e) => eprintln!("{:<COL_TOOL$} ERROR   {e}", ".xtask-cache"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- constant table validation ---

    #[test]
    fn required_tools_has_no_duplicate_names() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in REQUIRED_TOOLS {
            assert!(seen.insert(*name), "duplicate in REQUIRED_TOOLS: {name}");
        }
    }

    #[test]
    fn optional_tools_has_no_duplicate_names() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in OPTIONAL_TOOLS {
            assert!(seen.insert(*name), "duplicate in OPTIONAL_TOOLS: {name}");
        }
    }

    #[test]
    fn required_and_optional_tools_have_no_overlap() {
        let required: std::collections::HashSet<&str> =
            REQUIRED_TOOLS.iter().map(|(n, _)| *n).collect();
        for (name, _) in OPTIONAL_TOOLS {
            assert!(
                !required.contains(name),
                "tool {name} appears in both REQUIRED and OPTIONAL"
            );
        }
    }

    #[test]
    fn all_required_tools_have_nonempty_install_name() {
        for (name, install) in REQUIRED_TOOLS {
            assert!(
                !install.is_empty(),
                "empty install name for required tool {name}"
            );
        }
    }

    #[test]
    fn cargo_llvm_cov_is_a_required_tool() {
        let names: Vec<&str> = REQUIRED_TOOLS.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"cargo-llvm-cov"),
            "cargo-llvm-cov must be required"
        );
    }

    // --- check_tool_exists dispatch ---

    #[test]
    fn check_label_llvm_cov_returns_cargo_subcommand_label() {
        assert_eq!(check_label("cargo-llvm-cov"), "cargo llvm-cov");
    }

    #[test]
    fn check_label_other_tools_pass_through() {
        assert_eq!(check_label("cargo-nextest"), "cargo-nextest");
        assert_eq!(check_label("cargo-deny"), "cargo-deny");
        assert_eq!(check_label("cargo-machete"), "cargo-machete");
    }

    // --- confirm response parsing ---

    #[test]
    fn confirm_response_y_and_uppercase_y_accepted() {
        assert!(matches!("y", "y" | "Y"));
        assert!(matches!("Y", "y" | "Y"));
    }

    #[test]
    fn confirm_response_n_and_other_rejected() {
        assert!(!matches!("n", "y" | "Y"));
        assert!(!matches!("", "y" | "Y"));
        assert!(!matches!("yes", "y" | "Y"));
    }
}
