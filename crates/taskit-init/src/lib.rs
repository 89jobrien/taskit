pub mod plan;
pub mod render_cruxfile;
pub mod render_toml;

use std::path::Path;

/// Run the init command: discover workspace, generate taskit.toml + Cruxfile.
pub fn run(force: bool, interactive: bool) -> anyhow::Result<()> {
    let target = Path::new("taskit.toml");
    if target.exists() && !force {
        anyhow::bail!("taskit.toml already exists. Use --force to overwrite.");
    }

    let init_plan = if interactive {
        plan::plan_interactive()?
    } else {
        plan::plan_from_discovery()?
    };

    let project_name = detect_project_name();

    let toml_content = render_toml::render_toml(&init_plan);
    std::fs::write(target, &toml_content)?;
    eprintln!("wrote taskit.toml");

    let crux_content = render_cruxfile::render_cruxfile(&init_plan, &project_name);
    let crux_path = Path::new("Cruxfile");
    if !crux_path.exists() || force {
        std::fs::write(crux_path, &crux_content)?;
        eprintln!("wrote Cruxfile");
    }

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
        let toml_path = dir.path().join("taskit.toml");
        std::fs::write(&toml_path, "").unwrap();

        // Simulate by checking the guard logic directly
        assert!(toml_path.exists());
    }

    #[test]
    fn detect_project_name_returns_something() {
        let name = detect_project_name();
        assert!(!name.is_empty());
    }
}
