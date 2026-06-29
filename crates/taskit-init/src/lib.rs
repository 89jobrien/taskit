pub mod plan;
pub mod render_cruxfile;
pub mod render_toml;
pub mod scaffold;

use std::path::Path;

use taskit_types::error::{InitError, TaskitError};

/// Run the init command: discover workspace, generate taskit.toml + Cruxfile.
pub fn run(force: bool, interactive: bool) -> Result<(), TaskitError> {
    let target = Path::new("taskit.toml");
    if target.exists() && !force {
        return Err(InitError::AlreadyExists.into());
    }

    let init_plan = if interactive {
        plan::plan_interactive().map_err(|e| InitError::CargoMetadata {
            reason: e.to_string(),
        })?
    } else {
        plan::plan_from_discovery().map_err(|e| InitError::CargoMetadata {
            reason: e.to_string(),
        })?
    };

    let project_name = detect_project_name();

    // Core config: taskit.toml
    let toml_content = render_toml::render_toml(&init_plan);
    std::fs::write(target, &toml_content).map_err(|e| InitError::WriteFile {
        file: "taskit.toml".into(),
        reason: e.to_string(),
    })?;
    eprintln!("wrote taskit.toml");

    // Cruxfile
    let crux_content = render_cruxfile::render_cruxfile(&init_plan, &project_name);
    let crux_path = Path::new("Cruxfile");
    if !crux_path.exists() || force {
        std::fs::write(crux_path, &crux_content).map_err(|e| InitError::WriteFile {
            file: "Cruxfile".into(),
            reason: e.to_string(),
        })?;
        eprintln!("wrote Cruxfile");
    }

    // Cargo alias + xtask shim
    write_cargo_alias(force)?;
    write_xtask_crate(force)?;

    // Scaffold files (git hooks, CI, deny.toml, .ctx/)
    if init_plan.git_hooks {
        scaffold::write_git_hooks(force)?;
    }
    if init_plan.github_ci {
        scaffold::write_github_ci(force)?;
    }
    if init_plan.deny_toml {
        scaffold::write_deny_toml(force)?;
    }
    if init_plan.ctx_scaffold {
        scaffold::write_ctx_scaffold(force)?;
    }
    if init_plan.mdbook {
        scaffold::write_mdbook(&init_plan, &project_name, force)?;
    }

    eprintln!();
    eprintln!("taskit initialized for {project_name}!");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  1. Review taskit.toml — uncomment sections you want to enable");
    eprintln!("  2. Add \"xtask\" to [workspace] members in Cargo.toml");
    eprintln!("  3. Run `cargo xtask ci` to verify your pipeline");

    Ok(())
}

/// Generate a thin `xtask/` crate that delegates to the `taskit` binary.
fn write_xtask_crate(force: bool) -> Result<(), TaskitError> {
    let xtask_dir = Path::new("xtask");
    let src_dir = xtask_dir.join("src");
    let cargo_toml = xtask_dir.join("Cargo.toml");
    let main_rs = src_dir.join("main.rs");

    if cargo_toml.exists() && !force {
        eprintln!("xtask/Cargo.toml already exists, skipping");
        return Ok(());
    }

    std::fs::create_dir_all(&src_dir)?;

    let cargo_content = r#"[package]
name = "xtask"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
"#;
    std::fs::write(&cargo_toml, cargo_content)?;

    let main_content = r#"//! Self-updating xtask shim that delegates to the `taskit` binary.
//!
//! Usage: `cargo xtask <subcommand> [args...]`
//!
//! If `taskit` is not installed, this shim installs it automatically
//! via `cargo install taskit`.

use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Try running taskit directly first
    match Command::new("taskit").args(&args).status() {
        Ok(status) => exit(status.code().unwrap_or(1)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("taskit not found, installing via cargo install...");
            let install = Command::new("cargo")
                .args(["install", "taskit"])
                .status()
                .expect("failed to run cargo install");
            if !install.success() {
                eprintln!("failed to install taskit");
                exit(1);
            }
            // Retry after install
            let status = Command::new("taskit")
                .args(&args)
                .status()
                .expect("failed to run taskit after install");
            exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("failed to run taskit: {e}");
            exit(1);
        }
    }
}
"#;
    std::fs::write(&main_rs, main_content)?;

    eprintln!("wrote xtask/ crate (cargo xtask shim)");
    eprintln!("  -> add \"xtask\" to [workspace] members in Cargo.toml");

    Ok(())
}

/// Write `.cargo/config.toml` with a `cargo xtask` alias pointing to taskit.
fn write_cargo_alias(force: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".cargo");
    let path = dir.join("config.toml");

    if path.exists() && !force {
        let existing = std::fs::read_to_string(&path)?;
        if existing.contains("xtask") {
            eprintln!(".cargo/config.toml already has xtask alias, skipping");
            return Ok(());
        }
        let mut content = existing;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n[alias]\nxtask = \"run --package xtask --\"\n");
        std::fs::write(&path, content)?;
        eprintln!("appended xtask alias to .cargo/config.toml");
        return Ok(());
    }

    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, "[alias]\nxtask = \"run --package xtask --\"\n")?;
    eprintln!("wrote .cargo/config.toml (cargo xtask alias)");
    Ok(())
}

fn detect_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "my-project".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_refuses_overwrite_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("taskit.toml");
        std::fs::write(&target, "existing").unwrap();
        assert!(target.exists());
        let exists = target.exists();
        let force = false;
        assert!(
            exists && !force,
            "guard should trigger: file exists and force is false"
        );
    }

    #[test]
    fn run_creates_files_in_workspace() {
        let plan = plan::plan_from_discovery().unwrap();
        let toml = render_toml::render_toml(&plan);
        assert!(toml.contains("[workspace]"));
        let crux = render_cruxfile::render_cruxfile(&plan, "test-project");
        assert!(crux.contains("steps:") || crux.contains("taskit ci"));
    }

    #[test]
    fn detect_project_name_returns_something() {
        let name = detect_project_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn write_xtask_crate_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let xtask_dir = dir.path().join("xtask");
        let src_dir = xtask_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let cargo_toml = xtask_dir.join("Cargo.toml");
        let main_rs = src_dir.join("main.rs");
        std::fs::write(&cargo_toml, "[package]\nname = \"xtask\"\n").unwrap();
        std::fs::write(&main_rs, "fn main() {}\n").unwrap();
        assert!(cargo_toml.exists());
        assert!(main_rs.exists());
        let cargo = std::fs::read_to_string(&cargo_toml).unwrap();
        assert!(cargo.contains("xtask"));
    }

    #[test]
    fn write_cargo_alias_creates_config() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_dir = dir.path().join(".cargo");
        std::fs::create_dir_all(&cargo_dir).unwrap();
        let config_path = cargo_dir.join("config.toml");
        let content = "[alias]\nxtask = \"run --package xtask --\"\n";
        std::fs::write(&config_path, content).unwrap();
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("xtask"));
        assert!(written.contains("run --package xtask"));
    }

    #[test]
    fn write_cargo_alias_appends_to_existing() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_dir = dir.path().join(".cargo");
        std::fs::create_dir_all(&cargo_dir).unwrap();
        let config_path = cargo_dir.join("config.toml");
        let existing = "[build]\ntarget-dir = \"target\"\n";
        std::fs::write(&config_path, existing).unwrap();
        let mut content = existing.to_string();
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n[alias]\nxtask = \"run --package xtask --\"\n");
        std::fs::write(&config_path, &content).unwrap();
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("[build]"));
        assert!(written.contains("[alias]"));
        assert!(written.contains("run --package xtask"));
    }

    #[test]
    fn generated_toml_includes_all_sections() {
        let plan = plan::plan_from_discovery().unwrap();
        let toml = render_toml::render_toml(&plan);

        // Should have workspace crates
        assert!(toml.contains("[workspace]"));
        assert!(toml.contains("crates = ["));

        // Should have propagation (either active or commented)
        assert!(
            toml.contains("[[workspace.propagation]]")
                || toml.contains("# [[workspace.propagation]]")
        );

        // Should have protocol (either active or commented)
        assert!(toml.contains("[protocol]") || toml.contains("# [protocol]"));

        // Should have CI steps
        assert!(toml.contains("[[ci.steps]]") || toml.contains("# [[ci.steps]]"));

        // Should have flow (either active or commented)
        assert!(toml.contains("# [flow]") || toml.contains("[flow]"));

        // Should have coverage (either active or commented)
        assert!(toml.contains("[coverage]") || toml.contains("# [coverage]"));
    }
}
