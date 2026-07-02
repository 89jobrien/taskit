//! Execution context threaded through every command.
//!
//! `Ctx` replaces the former process-global `runner::DRY_RUN` / `SILENT`
//! statics with an injected dependency: it owns the [`Shell`], the parsed
//! [`Config`], and the run-time flags (`dry_run`, `output`, transient
//! `silent`). Command implementations receive `&Ctx` and route all shell
//! execution through its methods, so dry-run and output behaviour are visible
//! in signatures rather than read from ambient state.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use taskit_types::config::{
    CiConfig, Config, CoverageConfig, FlowConfig, ProtocolConfig, WorkspaceConfig,
};
use taskit_types::error::{TaskitError, TaskitResultExt};
use taskit_types::output_format::OutputFormat;
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
        if self.dry_run {
            taskit_output::taskit_dry!("{cmd}");
            return Ok(());
        }
        if self.silent.get() {
            let label = cmd.to_string();
            let out = cmd.quiet().output().err_context(&label)?;
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
        cmd.quiet().run().map_err(TaskitError::other)?;
        Ok(())
    }

    /// Run a shell command and capture its stdout/stderr.
    ///
    /// Returns `Ok(CapturedOutput)` even on non-zero exit (`success == false`).
    /// Returns `Err` only if the command cannot be spawned.
    pub fn run_capture(&self, cmd: Cmd<'_>) -> Result<CapturedOutput, TaskitError> {
        if self.dry_run {
            taskit_output::taskit_dry!("{cmd}");
            return Ok(CapturedOutput {
                stdout: String::new(),
                stderr: String::new(),
                success: true,
            });
        }
        let label = cmd.to_string();
        let out = cmd.quiet().ignore_status().output().err_context(&label)?;
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
