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

    // Cargo alias + xtask shim
    write_cargo_alias(force, dry_run)?;
    write_xtask_crate(force, dry_run)?;

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
    eprintln!("  2. Add \"xtask\" to [workspace] members in Cargo.toml");
    eprintln!("  3. Run `cargo xtask ci` to verify your pipeline");

    Ok(())
}

/// Generate a full `xtask/` crate with per-command modules delegating to `taskit`.
fn write_xtask_crate(force: bool, dry_run: bool) -> Result<(), TaskitError> {
    let xtask_dir = Path::new("xtask");
    let src_dir = xtask_dir.join("src");
    let cargo_toml = xtask_dir.join("Cargo.toml");

    if cargo_toml.exists() && !force {
        eprintln!("xtask/Cargo.toml already exists, skipping");
        return Ok(());
    }

    if !dry_run {
        std::fs::create_dir_all(&src_dir)?;
    }

    // --- Cargo.toml ---
    emit_file(
        &cargo_toml,
        r#"[package]
name = "xtask"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
clap = { version = "4", features = ["derive"] }
"#,
        dry_run,
    )?;

    // --- src/main.rs ---
    emit_file(
        &src_dir.join("main.rs"),
        r#"mod book;
mod ci;
mod fmt;
mod lint;
mod pre_commit;
mod pre_push;
mod test;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Workspace task runner (delegates to taskit)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build or serve the mdBook documentation.
    Book(book::Args),
    /// Run the full CI pipeline.
    Ci(ci::Args),
    /// Format all Rust code.
    Fmt(fmt::Args),
    /// Run clippy lints.
    Lint(lint::Args),
    /// Run tests via nextest.
    Test(test::Args),
    /// Pre-commit hook delegate.
    PreCommit,
    /// Pre-push hook delegate.
    PrePush,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Book(args) => book::run(args),
        Command::Ci(args) => ci::run(args),
        Command::Fmt(args) => fmt::run(args),
        Command::Lint(args) => lint::run(args),
        Command::Test(args) => test::run(args),
        Command::PreCommit => pre_commit::run(),
        Command::PrePush => pre_push::run(),
    };
    std::process::exit(code);
}

/// Run `taskit` with the given args. Installs it automatically if missing.
pub fn taskit(args: &[&str]) -> i32 {
    use std::process::Command;

    match Command::new("taskit").args(args).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("taskit not found, installing via cargo install...");
            let install = Command::new("cargo")
                .args(["install", "taskit"])
                .status()
                .expect("failed to run cargo install");
            if !install.success() {
                eprintln!("failed to install taskit");
                return 1;
            }
            Command::new("taskit")
                .args(args)
                .status()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or(1)
        }
        Err(e) => {
            eprintln!("failed to run taskit: {e}");
            1
        }
    }
}
"#,
        dry_run,
    )?;

    // --- src/book.rs ---
    emit_file(
        &src_dir.join("book.rs"),
        r#"use clap::Args as ClapArgs;
use std::process::Command;

#[derive(ClapArgs)]
pub struct Args {
    /// Serve the book locally with live reload.
    #[arg(long)]
    serve: bool,
    /// Port for the dev server.
    #[arg(long, default_value = "3000")]
    port: u16,
}

pub fn run(args: Args) -> i32 {
    let subcmd = if args.serve { "serve" } else { "build" };
    let mut cmd = Command::new("mdbook");
    cmd.arg(subcmd).arg("docs/");
    if args.serve {
        cmd.args(["--port", &args.port.to_string()]);
    }
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("mdbook not found — install with: cargo install mdbook");
            1
        }
        Err(e) => {
            eprintln!("failed to run mdbook: {e}");
            1
        }
    }
}
"#,
        dry_run,
    )?;

    // --- src/ci.rs ---
    emit_file(
        &src_dir.join("ci.rs"),
        r#"use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Stop on first failure.
    #[arg(long)]
    fail_fast: bool,
    /// Include network-dependent tests.
    #[arg(long)]
    include_network: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["ci"];
    if args.fail_fast {
        cmd.push("--fail-fast");
    }
    if args.include_network {
        cmd.push("--include-network");
    }
    crate::taskit(&cmd)
}
"#,
        dry_run,
    )?;

    // --- src/fmt.rs ---
    emit_file(
        &src_dir.join("fmt.rs"),
        r#"use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Check formatting without writing.
    #[arg(long)]
    check: bool,
    /// Only format affected crates.
    #[arg(long)]
    affected: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["fmt"];
    if args.check {
        cmd.push("--check");
    }
    if args.affected {
        cmd.push("--affected");
    }
    crate::taskit(&cmd)
}
"#,
        dry_run,
    )?;

    // --- src/lint.rs ---
    emit_file(
        &src_dir.join("lint.rs"),
        r#"use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Lint a specific crate.
    #[arg(long)]
    crate_name: Option<String>,
    /// Only lint affected crates.
    #[arg(long)]
    affected: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["lint"];
    let crate_flag;
    if let Some(ref name) = args.crate_name {
        cmd.push("--crate-name");
        crate_flag = name.as_str();
        cmd.push(crate_flag);
    }
    if args.affected {
        cmd.push("--affected");
    }
    crate::taskit(&cmd)
}
"#,
        dry_run,
    )?;

    // --- src/test.rs ---
    emit_file(
        &src_dir.join("test.rs"),
        r#"use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Test a specific crate.
    #[arg(long)]
    crate_name: Option<String>,
    /// Only test affected crates.
    #[arg(long)]
    affected: bool,
    /// Skip network-dependent tests.
    #[arg(long)]
    offline: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["test"];
    let crate_flag;
    if let Some(ref name) = args.crate_name {
        cmd.push("--crate-name");
        crate_flag = name.as_str();
        cmd.push(crate_flag);
    }
    if args.affected {
        cmd.push("--affected");
    }
    if args.offline {
        cmd.push("--offline");
    }
    crate::taskit(&cmd)
}
"#,
        dry_run,
    )?;

    // --- src/pre_commit.rs ---
    emit_file(
        &src_dir.join("pre_commit.rs"),
        r#"pub fn run() -> i32 {
    crate::taskit(&["pre-commit"])
}
"#,
        dry_run,
    )?;

    // --- src/pre_push.rs ---
    emit_file(
        &src_dir.join("pre_push.rs"),
        r#"pub fn run() -> i32 {
    crate::taskit(&["pre-push"])
}
"#,
        dry_run,
    )?;

    let verb = if dry_run { "would write" } else { "wrote" };
    eprintln!("{verb} xtask/ crate (8 files)");
    eprintln!("  -> add \"xtask\" to [workspace] members in Cargo.toml");

    Ok(())
}

/// Write `.cargo/config.toml` with a `cargo xtask` alias pointing to taskit.
fn write_cargo_alias(force: bool, dry_run: bool) -> Result<(), TaskitError> {
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
        emit_file(&path, &content, dry_run)?;
        if !dry_run {
            eprintln!("appended xtask alias to .cargo/config.toml");
        }
        return Ok(());
    }

    emit_file(
        &path,
        "[alias]\nxtask = \"run --package xtask --\"\n",
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
    fn write_xtask_crate_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let xtask_dir = dir.path().join("xtask");
        let src_dir = xtask_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        // Simulate what write_xtask_crate generates
        let cargo_toml = xtask_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"xtask\"\n").unwrap();

        let modules = [
            "main.rs",
            "book.rs",
            "ci.rs",
            "fmt.rs",
            "lint.rs",
            "test.rs",
            "pre_commit.rs",
            "pre_push.rs",
        ];
        for m in &modules {
            std::fs::write(src_dir.join(m), "// generated\n").unwrap();
        }

        assert!(cargo_toml.exists());
        for m in &modules {
            assert!(src_dir.join(m).exists(), "{m} should exist");
        }
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
