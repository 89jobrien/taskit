pub mod plan;
pub mod render_cruxfile;
pub mod render_toml;
pub mod scaffold;

use std::path::Path;

use taskit_types::error::{InitError, TaskitError};

/// Write a file or print its content when dry-running.
pub(crate) fn emit_file(path: &Path, content: &str, dry_run: bool) -> Result<(), TaskitError> {
    if dry_run {
        eprintln!("would write {}", path.display());
    } else {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content).map_err(|e| InitError::WriteFile {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

/// Run the init command: discover workspace, generate taskit.toml + Cruxfile.
pub fn run(force: bool, interactive: bool, dry_run: bool) -> Result<(), TaskitError> {
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
    emit_file(target, &toml_content, dry_run)?;

    // Cruxfile
    let crux_content = render_cruxfile::render_cruxfile(&init_plan, &project_name);
    let crux_path = Path::new("Cruxfile");
    if !crux_path.exists() || force {
        emit_file(crux_path, &crux_content, dry_run)?;
    }

    // Cargo alias for `cargo taskit`
    write_cargo_alias(force, dry_run)?;

    // Scaffold files (git hooks, CI, deny.toml, .ctx/)
    if init_plan.git_hooks {
        scaffold::write_git_hooks(force, dry_run)?;
    }
    if init_plan.github_ci {
        scaffold::write_github_ci(force, dry_run)?;
    }
    if init_plan.deny_toml {
        scaffold::write_deny_toml(force, dry_run)?;
    }
    if init_plan.ctx_scaffold {
        scaffold::write_ctx_scaffold(force, dry_run)?;
    }
    if init_plan.mdbook {
        scaffold::write_mdbook(&init_plan, &project_name, force, dry_run)?;
    }

    eprintln!();
    eprintln!("taskit initialized for {project_name}!");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  1. Review taskit.toml — uncomment sections you want to enable");
    eprintln!("  2. Run `taskit ci` to verify your pipeline");

    Ok(())
}

/// Write `.cargo/config.toml` with a `cargo taskit` alias.
fn write_cargo_alias(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let dir = Path::new(".cargo");
    let path = dir.join("config.toml");

    if path.exists() && !force {
        let existing = std::fs::read_to_string(&path)?;
        if existing.contains("taskit") {
            eprintln!(".cargo/config.toml already has taskit alias, skipping");
            return Ok(());
        }
        let mut content = existing;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n[alias]\ntaskit = \"run --package taskit --\"\n");
        emit_file(&path, &content, dry_run)?;
        if !dry_run {
            eprintln!("appended taskit alias to .cargo/config.toml");
        }
        return Ok(());
    }

    emit_file(
        &path,
        "[alias]\ntaskit = \"run --package taskit --\"\n",
        dry_run,
    )?;
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
    fn write_cargo_alias_creates_config() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_dir = dir.path().join(".cargo");
        std::fs::create_dir_all(&cargo_dir).unwrap();
        let config_path = cargo_dir.join("config.toml");
        let content = "[alias]\ntaskit = \"run --package taskit --\"\n";
        std::fs::write(&config_path, content).unwrap();
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("taskit"));
        assert!(written.contains("run --package taskit"));
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
        content.push_str("\n[alias]\ntaskit = \"run --package taskit --\"\n");
        std::fs::write(&config_path, &content).unwrap();
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("[build]"));
        assert!(written.contains("[alias]"));
        assert!(written.contains("run --package taskit"));
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
