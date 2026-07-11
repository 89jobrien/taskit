//! Execution context threaded through every command.
//!
//! `Ctx` replaces the former process-global `runner::DRY_RUN` / `SILENT`
//! statics with an injected dependency: it owns the [`Shell`], the parsed
//! [`Config`], and the run-time flags (`dry_run`, `output`, transient
//! `silent`). Command implementations receive `&Ctx` and route all shell
//! execution through its methods, so dry-run and output behaviour are visible
//! in signatures rather than read from ambient state.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use taskit_types::config::{
    CiConfig, CleanConfig, Config, CoverageConfig, FlowConfig, InspectConfig, ProtocolConfig,
    ReleaseConfig, WorkspaceConfig,
};
use taskit_types::error::{TaskitError, TaskitResultExt};
use taskit_types::output_format::OutputFormat;
use taskit_types::step::{CommandRecord, PipelineRunContext};
use xshell::{Cmd, Shell};

/// Captured output from a command execution.
pub struct CapturedOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Injected execution context for all commands.
pub struct Ctx {
    /// Shell used to build and run commands.
    pub sh: Shell,
    /// Resolved workspace root (also the process cwd once dispatch begins).
    pub root: PathBuf,
    /// Parsed workspace configuration.
    pub config: Config,
    /// When true, commands print what they would run instead of running it.
    pub dry_run: bool,
    /// Output format for commands that render a report.
    pub output: OutputFormat,
    /// Transient: suppress child stdout/stderr for the duration of a closure.
    silent: Cell<bool>,
    /// Commands executed through this context, used for step diagnostics.
    command_log: RefCell<Vec<CommandRecord>>,
}

impl Ctx {
    /// Build a context from a shell, resolved root, parsed config, and flags.
    pub fn new(
        sh: Shell,
        root: PathBuf,
        config: Config,
        dry_run: bool,
        output: OutputFormat,
    ) -> Self {
        Self {
            sh,
            root,
            config,
            dry_run,
            output,
            silent: Cell::new(false),
            command_log: RefCell::new(Vec::new()),
        }
    }

    // ── config accessors ──────────────────────────────────────────────────

    /// The resolved workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The `[workspace]` section.
    pub fn ws(&self) -> &WorkspaceConfig {
        &self.config.workspace
    }

    /// The optional `[protocol]` section.
    pub fn proto(&self) -> Option<&ProtocolConfig> {
        self.config.protocol.as_ref()
    }

    /// The optional `[coverage]` section.
    pub fn cov(&self) -> Option<&CoverageConfig> {
        self.config.coverage.as_ref()
    }

    /// The optional `[ci]` section.
    pub fn ci(&self) -> Option<&CiConfig> {
        self.config.ci.as_ref()
    }

    /// The optional `[inspect]` section.
    pub fn inspect(&self) -> Option<&InspectConfig> {
        self.config.inspect.as_ref()
    }

    /// The optional `[clean]` section.
    pub fn clean_config(&self) -> Option<&CleanConfig> {
        self.config.clean.as_ref()
    }

    /// The optional `[release]` section.
    pub fn release_config(&self) -> Option<&ReleaseConfig> {
        self.config.release.as_ref()
    }

    /// The `[flow]` section, or defaults when unset.
    pub fn flow(&self) -> FlowConfig {
        self.config.flow.clone().unwrap_or_default()
    }

    // ── execution ─────────────────────────────────────────────────────────

    /// Run a shell command, or in dry-run mode print it instead.
    ///
    /// `.quiet()` suppresses xshell's `$ cmd args` echo; progress spinners
    /// provide the user-facing feedback instead. When [`Ctx::with_silent`] is
    /// active, stdout/stderr are captured and discarded on success, or
    /// attached to the error on failure.
    pub fn run(&self, cmd: Cmd<'_>) -> Result<(), TaskitError> {
        let label = cmd.to_string();
        if self.dry_run {
            self.record_command(CommandRecord {
                command: label.clone(),
                success: None,
                exit_code: None,
            });
            taskit_output::taskit_dry!("{label}");
            return Ok(());
        }
        if self.silent.get() {
            let out = cmd.quiet().output().err_context(&label)?;
            self.record_command(CommandRecord {
                command: label.clone(),
                success: Some(out.status.success()),
                exit_code: out.status.code(),
            });
            if !out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(TaskitError::other(format!(
                    "{label} failed (exit {})\n{stdout}{stderr}",
                    out.status.code().unwrap_or(-1)
                )));
            }
            return Ok(());
        }
        let result = cmd.quiet().run().map_err(TaskitError::other);
        self.record_command(CommandRecord {
            command: label,
            success: Some(result.is_ok()),
            exit_code: None,
        });
        result?;
        Ok(())
    }

    /// Run a shell command and capture its stdout/stderr.
    ///
    /// Returns `Ok(CapturedOutput)` even on non-zero exit (`success == false`).
    /// Returns `Err` only if the command cannot be spawned.
    pub fn run_capture(&self, cmd: Cmd<'_>) -> Result<CapturedOutput, TaskitError> {
        let label = cmd.to_string();
        if self.dry_run {
            self.record_command(CommandRecord {
                command: label.clone(),
                success: None,
                exit_code: None,
            });
            taskit_output::taskit_dry!("{label}");
            return Ok(CapturedOutput {
                stdout: String::new(),
                stderr: String::new(),
                success: true,
            });
        }
        let out = cmd.quiet().ignore_status().output().err_context(&label)?;
        self.record_command(CommandRecord {
            command: label,
            success: Some(out.status.success()),
            exit_code: out.status.code(),
        });
        Ok(CapturedOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            success: out.status.success(),
        })
    }

    /// Best-effort run (ignores errors), or in dry-run mode print instead.
    #[allow(dead_code)]
    pub fn run_ok(&self, cmd: Cmd<'_>) {
        if self.dry_run {
            taskit_output::taskit_dry!("{cmd}");
        } else {
            let _ = cmd.run();
        }
    }

    /// Suppress child stdout/stderr for the duration of `f`.
    ///
    /// Used by `taskit quick` so only progress spinners are visible. The
    /// captured output is attached to the error on failure so nothing is lost.
    pub fn with_silent<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let prev = self.silent.replace(true);
        let result = f();
        self.silent.set(prev);
        result
    }

    /// Return the current command-log length before a step starts.
    pub fn command_capture_start(&self) -> usize {
        self.command_log.borrow().len()
    }

    /// Return command records appended since `start_index`.
    pub fn command_capture_finish(&self, start_index: usize) -> Vec<CommandRecord> {
        self.command_log
            .borrow()
            .iter()
            .skip(start_index)
            .cloned()
            .collect()
    }

    /// Collect best-effort run context for pipeline diagnostics.
    pub fn pipeline_run_context(&self) -> PipelineRunContext {
        PipelineRunContext {
            taskit_binary: std::env::current_exe()
                .ok()
                .map(|path| path.display().to_string()),
            taskit_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_root: self.root.display().to_string(),
            git_sha: command_output(&self.root, "git", &["rev-parse", "HEAD"]),
            rustc_version: command_output(&self.root, "rustc", &["--version"]),
            cargo_version: command_output(&self.root, "cargo", &["--version"]),
            workspace_members: workspace_member_names(&self.root),
        }
    }

    fn record_command(&self, record: CommandRecord) {
        self.command_log.borrow_mut().push(record);
    }

    /// Test-only context: fresh shell, default config, executing (not dry-run).
    #[cfg(test)]
    pub fn test() -> Self {
        Self::new(
            Shell::new().expect("create shell"),
            PathBuf::from("."),
            Config::default(),
            false,
            OutputFormat::Human,
        )
    }
}

fn command_output(current_dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn workspace_member_names(root: &Path) -> Vec<String> {
    let Ok(metadata) = cargo_metadata::MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .exec()
    else {
        return Vec::new();
    };

    let mut names: Vec<String> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.iter().find(|pkg| &pkg.id == id))
        .map(|pkg| pkg.name.clone())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_reflect_config() {
        let ctx = Ctx::test();
        // Default config: workspace present, optional sections absent.
        assert!(ctx.proto().is_none());
        assert!(ctx.cov().is_none());
        assert!(ctx.ci().is_none());
        // flow() always yields a value (defaults when unset).
        let _ = ctx.flow();
        let _ = ctx.ws();
    }

    #[test]
    fn dry_run_flag_is_readable() {
        let ctx = Ctx::test();
        assert!(!ctx.dry_run);
    }

    #[test]
    fn dry_run_commands_are_recorded() {
        let ctx = Ctx::new(
            Shell::new().expect("create shell"),
            PathBuf::from("."),
            Config::default(),
            true,
            OutputFormat::Human,
        );
        let start = ctx.command_capture_start();
        ctx.run(xshell::cmd!(ctx.sh, "cargo --version")).unwrap();
        let records = ctx.command_capture_finish(start);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].success, None);
        assert!(records[0].command.contains("cargo --version"));
    }

    #[test]
    fn pipeline_run_context_has_required_fields() {
        let ctx = Ctx::test();
        let context = ctx.pipeline_run_context();
        assert_eq!(context.taskit_version, env!("CARGO_PKG_VERSION"));
        assert!(!context.workspace_root.is_empty());
    }

    #[test]
    fn with_silent_restores_previous_state() {
        let ctx = Ctx::test();
        assert!(!ctx.silent.get());
        ctx.with_silent(|| {
            assert!(ctx.silent.get(), "silent must be set inside the closure");
        });
        assert!(
            !ctx.silent.get(),
            "silent must be restored after the closure"
        );
    }

    #[test]
    fn with_silent_nests() {
        let ctx = Ctx::test();
        ctx.with_silent(|| {
            ctx.with_silent(|| assert!(ctx.silent.get()));
            assert!(
                ctx.silent.get(),
                "inner restore must not clear outer silent"
            );
        });
        assert!(!ctx.silent.get());
    }
}
