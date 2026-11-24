//! Shared taskit error types, diagnostics, and result-context helpers.

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// Top-level error type used across taskit commands and crates.
#[derive(Debug, Error, Diagnostic)]
pub enum TaskitError {
    /// Configuration parsing or validation error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] ConfigError),

    /// Pipeline execution error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Pipeline(#[from] PipelineError),

    /// Protocol drift or lockfile error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Protocol(#[from] ProtocolError),

    /// Workspace initialization error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Init(#[from] InitError),

    /// Branch flow automation error.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Flow(#[from] FlowError),

    /// Underlying I/O error.
    #[error("io error: {0}")]
    #[diagnostic(code(taskit::io))]
    Io(#[from] std::io::Error),

    /// Internal or contextualized error from dynamic sources.
    #[error("{0}")]
    #[diagnostic(code(taskit::internal))]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl TaskitError {
    /// Create an `Other` variant from any display-able message.
    pub fn other(msg: impl std::fmt::Display) -> Self {
        TaskitError::Other(msg.to_string().into())
    }
}

/// Errors related to reading and validating taskit configuration.
#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    /// Config file could not be found.
    #[error("config file not found: {path}")]
    #[diagnostic(
        code(taskit::config::not_found),
        help("run `taskit init` to generate taskit.toml")
    )]
    NotFound {
        /// Config path that could not be found.
        path: String,
    },

    /// Config file exists but failed to parse.
    #[error("failed to parse config")]
    #[diagnostic(code(taskit::config::parse))]
    Parse {
        /// Source content used by diagnostics.
        #[source_code]
        src: NamedSource<String>,
        /// Span of the parse failure.
        #[label("parse error here")]
        span: SourceSpan,
        /// Parsing error returned by TOML/serde.
        #[source]
        reason: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Parsed config failed semantic validation.
    #[error("invalid config: {message}")]
    #[diagnostic(code(taskit::config::invalid), help("{hint}"))]
    Invalid {
        /// Validation error message.
        message: String,
        /// Suggested remediation hint.
        hint: String,
    },
}

/// Errors produced while running a command pipeline.
#[derive(Debug, Error, Diagnostic)]
pub enum PipelineError {
    /// One or more steps failed.
    #[error("pipeline failed: {failed_count} step(s) failed")]
    #[diagnostic(
        code(taskit::pipeline::failed),
        help("fix the failing steps above, then re-run")
    )]
    Failed {
        /// Number of failed steps.
        failed_count: usize,
        /// Rendered pipeline summary source.
        #[source_code]
        src: NamedSource<String>,
        /// Span covering the failed pipeline summary.
        #[label("pipeline result")]
        span: SourceSpan,
        /// Per-step nested errors.
        #[related]
        step_errors: Vec<StepError>,
    },

    /// A gate step failed, so execution stopped.
    #[error("gate '{name}' failed, aborting pipeline")]
    #[diagnostic(
        code(taskit::pipeline::gate_failed),
        help("gates are mandatory — fix before continuing")
    )]
    GateFailed {
        /// Name of the failing gate step.
        name: String,
        /// Optional underlying cause from the step.
        #[source]
        reason: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Errors related to protocol drift lockfile enforcement.
#[derive(Debug, Error, Diagnostic)]
pub enum ProtocolError {
    /// A tracked surface hash differs from lockfile expectation.
    #[error("protocol drift detected in surface '{name}'")]
    #[diagnostic(
        code(taskit::protocol::drift),
        help("run `taskit check-protocol-drift --update` to accept")
    )]
    Drift {
        /// Surface name.
        name: String,
        /// Hash value from lockfile.
        expected: String,
        /// Newly computed hash value.
        actual: String,
    },

    /// Lockfile path does not exist.
    #[error("lockfile not found: {path}")]
    #[diagnostic(
        code(taskit::protocol::lockfile_missing),
        help("run `taskit check-protocol-drift --update` to generate")
    )]
    LockfileMissing {
        /// Missing lockfile path.
        path: String,
    },

    /// Lockfile schema/content is stale or malformed.
    #[error("lockfile is stale")]
    #[diagnostic(
        code(taskit::protocol::stale),
        help("re-run `taskit check-protocol-drift --update`")
    )]
    Stale,
}

/// Errors raised by `taskit init` workflows.
#[derive(Debug, Error, Diagnostic)]
pub enum InitError {
    /// Target files already exist and overwrite was not requested.
    #[error("taskit.toml already exists")]
    #[diagnostic(code(taskit::init::exists), help("use --force to overwrite"))]
    AlreadyExists,

    /// Cargo metadata discovery failed.
    #[error("cargo metadata failed: {reason}")]
    #[diagnostic(code(taskit::init::metadata))]
    CargoMetadata {
        /// Human-readable error from metadata collection.
        reason: String,
    },

    /// A generated file could not be written.
    #[error("failed to write {file}: {reason}")]
    #[diagnostic(code(taskit::init::write))]
    WriteFile {
        /// Path of the file that failed to write.
        file: String,
        /// Human-readable write failure reason.
        reason: String,
    },
}

/// Errors raised by the branch promotion flow.
#[derive(Debug, Error, Diagnostic)]
#[non_exhaustive]
pub enum FlowError {
    /// Command ran on the wrong branch for the requested operation.
    #[error("not on expected branch: expected '{expected}', got '{actual}'")]
    #[diagnostic(
        code(taskit::flow::wrong_branch),
        help("switch to '{expected}' before running this command")
    )]
    WrongBranch {
        /// Branch required for this operation.
        expected: String,
        /// Current checked-out branch.
        actual: String,
    },

    /// Current branch is outside the configured flow branch set.
    #[error("branch '{branch}' is not a flow branch")]
    #[diagnostic(
        code(taskit::flow::not_a_flow_branch),
        help("flow promote must be run on {develop}, {staging}, or {release}")
    )]
    NotAFlowBranch {
        /// Current branch name.
        branch: String,
        /// Configured development branch.
        develop: String,
        /// Configured staging branch.
        staging: String,
        /// Configured release branch.
        release: String,
    },

    /// Direct commits are blocked on protected branches.
    #[error("branch '{branch}' is protected -- direct commits are blocked")]
    #[diagnostic(
        code(taskit::flow::protected),
        help("commit to '{staging}' and use `taskit flow promote`")
    )]
    ProtectedBranch {
        /// Protected branch name.
        branch: String,
        /// Suggested staging branch for commits.
        staging: String,
    },

    /// Required branch is missing from the repository.
    #[error("branch '{branch}' does not exist")]
    #[diagnostic(
        code(taskit::flow::missing_branch),
        help("create it with: git branch {branch}")
    )]
    MissingBranch {
        /// Missing branch name.
        branch: String,
    },

    /// Worktree contains uncommitted changes.
    #[error("branch '{branch}' has uncommitted changes")]
    #[diagnostic(
        code(taskit::flow::dirty),
        help("commit or stash changes before flow operations")
    )]
    DirtyWorktree {
        /// Branch where uncommitted changes were detected.
        branch: String,
    },

    /// A git merge operation failed.
    #[error("merge failed: {reason}")]
    #[diagnostic(code(taskit::flow::merge_failed))]
    MergeFailed {
        /// Underlying merge failure reason.
        reason: String,
    },

    /// Automatic conflict resolution could not complete safely.
    #[error("merge conflict needs human review: {path} — {reason}")]
    #[diagnostic(
        code(taskit::flow::needs_human),
        help("resolve manually, then run `taskit flow finish`")
    )]
    NeedsHuman {
        /// File path that requires manual conflict resolution.
        path: String,
        /// Reason automatic resolution stopped.
        reason: String,
    },

    /// CI gate failed during flow promotion.
    #[error("CI failed on release: {}", failed.join(", "))]
    #[diagnostic(
        code(taskit::flow::ci_failed),
        help("fix the failing steps, then re-run `taskit flow auto`")
    )]
    CiFailed {
        /// Names of CI steps that failed.
        failed: Vec<String>,
    },

    /// Push to remote failed after local promotion succeeded.
    #[error("push to '{remote}' failed: {reason}")]
    #[diagnostic(
        code(taskit::flow::push_failed),
        help(
            "branches are merged locally — fix the remote issue, then push manually or re-run `taskit flow auto`"
        )
    )]
    PushFailed {
        /// Remote name that rejected the push.
        remote: String,
        /// Underlying push failure reason.
        reason: String,
    },
}

/// Error details for one failing pipeline step.
#[derive(Debug, Error, Diagnostic)]
#[error("step \"{name}\" failed")]
#[diagnostic(severity(error))]
pub struct StepError {
    /// Step name.
    pub name: String,
    /// Optional extra detail message.
    #[help]
    pub detail: Option<String>,
}

/// Ergonomic error-context mapping for Result types.
///
/// Replaces `.map_err(|e| TaskitError::other(format!("msg: {e}")))`
/// with `.err_context("msg")?`.
pub trait TaskitResultExt<T> {
    /// Convert any error into `TaskitError::Other` with static context text.
    fn err_context(self, msg: &str) -> Result<T, TaskitError>;
    /// Convert any error into `TaskitError::Other` with lazily-built context.
    fn err_context_with<F: FnOnce() -> String>(self, f: F) -> Result<T, TaskitError>;
}

impl<T, E: std::fmt::Display> TaskitResultExt<T> for Result<T, E> {
    fn err_context(self, msg: &str) -> Result<T, TaskitError> {
        self.map_err(|e| TaskitError::other(format!("{msg}: {e}")))
    }

    fn err_context_with<F: FnOnce() -> String>(self, f: F) -> Result<T, TaskitError> {
        self.map_err(|e| TaskitError::other(format!("{}: {e}", f())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_not_found_display() {
        let err = TaskitError::Config(ConfigError::NotFound {
            path: "taskit.toml".into(),
        });
        assert!(
            err.to_string().contains("config file not found"),
            "got: {err}"
        );
    }

    #[test]
    fn config_not_found_diagnostic_code() {
        let err = ConfigError::NotFound {
            path: "taskit.toml".into(),
        };
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::config::not_found");
    }

    #[test]
    fn taskit_error_other_constructs_internal_error() {
        let err = TaskitError::other("custom failure");
        assert!(matches!(err, TaskitError::Other(_)));
        assert_eq!(err.to_string(), "custom failure");
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::internal");
    }

    #[test]
    fn taskit_error_io_display_and_diagnostic_code() {
        let err = TaskitError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        assert!(err.to_string().contains("io error"), "got: {err}");
        assert!(err.to_string().contains("missing"), "got: {err}");
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::io");
    }

    #[test]
    fn pipeline_failed_display() {
        let err = TaskitError::Pipeline(PipelineError::Failed {
            failed_count: 2,
            src: NamedSource::new("summary", "FAIL lint\nFAIL test".to_string()),
            span: (0, 18).into(),
            step_errors: vec![
                StepError {
                    name: "lint".into(),
                    detail: Some("clippy warnings".into()),
                },
                StepError {
                    name: "test".into(),
                    detail: None,
                },
            ],
        });
        assert!(err.to_string().contains("2 step(s) failed"), "got: {err}");
    }

    #[test]
    fn pipeline_failed_diagnostic_code() {
        let err = PipelineError::Failed {
            failed_count: 1,
            src: NamedSource::new("summary", "FAIL x".to_string()),
            span: (0, 6).into(),
            step_errors: vec![],
        };
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::pipeline::failed");
    }

    #[test]
    fn step_error_display() {
        let err = StepError {
            name: "lint".into(),
            detail: Some("too many warnings".into()),
        };
        assert!(err.to_string().contains("lint"));
    }

    #[test]
    fn gate_failed_display() {
        let err = PipelineError::GateFailed {
            name: "preflight".into(),
            reason: None,
        };
        assert!(err.to_string().contains("gate 'preflight' failed"));
    }

    #[test]
    fn protocol_drift_display() {
        let err = TaskitError::Protocol(ProtocolError::Drift {
            name: "core-api".into(),
            expected: "abc123".into(),
            actual: "def456".into(),
        });
        assert!(
            err.to_string().contains("protocol drift detected"),
            "got: {err}"
        );
    }

    #[test]
    fn protocol_drift_diagnostic_code() {
        let err = ProtocolError::Drift {
            name: "core-api".into(),
            expected: "abc".into(),
            actual: "def".into(),
        };
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::protocol::drift");
    }

    #[test]
    fn protocol_lockfile_missing_display() {
        let err = ProtocolError::LockfileMissing {
            path: "taskit-protocol.lock".into(),
        };
        assert!(err.to_string().contains("lockfile not found"));
    }

    #[test]
    fn protocol_stale_display_and_diagnostic_code() {
        let err = ProtocolError::Stale;
        assert_eq!(err.to_string(), "lockfile is stale");
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::protocol::stale");
    }

    #[test]
    fn init_already_exists_display() {
        let err = TaskitError::Init(InitError::AlreadyExists);
        assert!(err.to_string().contains("already exists"), "got: {err}");
    }

    #[test]
    fn init_already_exists_diagnostic_code() {
        let err = InitError::AlreadyExists;
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::init::exists");
    }

    #[test]
    fn init_cargo_metadata_display() {
        let err = InitError::CargoMetadata {
            reason: "not a cargo workspace".into(),
        };
        assert!(err.to_string().contains("cargo metadata failed"));
    }

    #[test]
    fn init_write_file_display_and_diagnostic_code() {
        let err = InitError::WriteFile {
            file: "taskit.toml".into(),
            reason: "permission denied".into(),
        };
        assert!(err.to_string().contains("failed to write taskit.toml"));
        assert!(err.to_string().contains("permission denied"));
        let code = err.code().expect("should have diagnostic code");
        assert_eq!(code.to_string(), "taskit::init::write");
    }

    #[test]
    fn flow_errors_display_debug_and_expose_diagnostic_codes() {
        let cases = [
            (
                FlowError::WrongBranch {
                    expected: "staging".into(),
                    actual: "main".into(),
                },
                "not on expected branch",
                "WrongBranch",
                "taskit::flow::wrong_branch",
            ),
            (
                FlowError::ProtectedBranch {
                    branch: "main".into(),
                    staging: "staging".into(),
                },
                "protected",
                "ProtectedBranch",
                "taskit::flow::protected",
            ),
            (
                FlowError::MissingBranch {
                    branch: "release".into(),
                },
                "does not exist",
                "MissingBranch",
                "taskit::flow::missing_branch",
            ),
            (
                FlowError::DirtyWorktree {
                    branch: "staging".into(),
                },
                "uncommitted changes",
                "DirtyWorktree",
                "taskit::flow::dirty",
            ),
            (
                FlowError::MergeFailed {
                    reason: "conflict".into(),
                },
                "merge failed",
                "MergeFailed",
                "taskit::flow::merge_failed",
            ),
        ];

        for (err, expected_display, expected_debug, expected_code) in cases {
            assert!(
                err.to_string().contains(expected_display),
                "expected {expected_display:?} in {err}"
            );
            assert!(
                format!("{err:?}").contains(expected_debug),
                "expected {expected_debug:?} in {err:?}"
            );
            let code = err.code().expect("should have diagnostic code");
            assert_eq!(code.to_string(), expected_code);
        }
    }

    #[test]
    fn flow_needs_human_display_and_code() {
        let err = FlowError::NeedsHuman {
            path: "src/lib.rs".into(),
            reason: "too complex".into(),
        };
        assert!(err.to_string().contains("src/lib.rs"));
        assert!(err.to_string().contains("too complex"));
        let code = err.code().expect("diagnostic code");
        assert_eq!(code.to_string(), "taskit::flow::needs_human");
    }

    #[test]
    fn flow_ci_failed_display_and_code() {
        let err = FlowError::CiFailed {
            failed: vec!["lint".into(), "test".into()],
        };
        assert!(err.to_string().contains("lint"));
        assert!(err.to_string().contains("test"));
        let code = err.code().expect("diagnostic code");
        assert_eq!(code.to_string(), "taskit::flow::ci_failed");
    }

    #[test]
    fn config_invalid_display() {
        let err = ConfigError::Invalid {
            message: "missing workspace".into(),
            hint: "add [workspace] section".into(),
        };
        assert!(err.to_string().contains("missing workspace"));
    }

    #[test]
    fn result_ext_ok_passthrough() {
        let r: Result<i32, std::io::Error> = Ok(42);
        assert_eq!(r.err_context("should not matter").unwrap(), 42);
    }

    #[test]
    fn result_ext_err_wraps_message() {
        let r: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        let err = r.err_context("reading file").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("reading file"), "got: {msg}");
        assert!(msg.contains("gone"), "got: {msg}");
    }

    #[test]
    fn result_ext_lazy_not_called_on_ok() {
        use std::cell::Cell;
        let called = Cell::new(false);
        let r: Result<i32, std::io::Error> = Ok(1);
        let _ = r.err_context_with(|| {
            called.set(true);
            "lazy".into()
        });
        assert!(!called.get());
    }

    #[test]
    fn result_ext_lazy_called_on_err() {
        let r: Result<(), String> = Err("bad".into());
        let err = r.err_context_with(|| "lazy context".into()).unwrap_err();
        assert!(err.to_string().contains("lazy context"));
    }
}
