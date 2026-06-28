use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum TaskitError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Pipeline(#[from] PipelineError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Protocol(#[from] ProtocolError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Init(#[from] InitError),

    #[error("io error: {0}")]
    #[diagnostic(code(taskit::io))]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error("config file not found: {path}")]
    #[diagnostic(
        code(taskit::config::not_found),
        help("run `taskit init` to generate taskit.toml")
    )]
    NotFound { path: String },

    #[error("failed to parse config")]
    #[diagnostic(code(taskit::config::parse))]
    Parse {
        #[source_code]
        src: NamedSource<String>,
        #[label("parse error here")]
        span: SourceSpan,
        #[source]
        reason: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("invalid config: {message}")]
    #[diagnostic(code(taskit::config::invalid), help("{hint}"))]
    Invalid { message: String, hint: String },
}

#[derive(Debug, Error, Diagnostic)]
pub enum PipelineError {
    #[error("pipeline failed: {failed_count} step(s) failed")]
    #[diagnostic(
        code(taskit::pipeline::failed),
        help("fix the failing steps above, then re-run")
    )]
    Failed {
        failed_count: usize,
        #[source_code]
        src: NamedSource<String>,
        #[label("pipeline result")]
        span: SourceSpan,
        #[related]
        step_errors: Vec<StepError>,
    },

    #[error("gate '{name}' failed, aborting pipeline")]
    #[diagnostic(
        code(taskit::pipeline::gate_failed),
        help("gates are mandatory — fix before continuing")
    )]
    GateFailed {
        name: String,
        #[source]
        reason: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

#[derive(Debug, Error, Diagnostic)]
pub enum ProtocolError {
    #[error("protocol drift detected in surface '{name}'")]
    #[diagnostic(
        code(taskit::protocol::drift),
        help("run `taskit check-protocol-drift --update` to accept")
    )]
    Drift {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("lockfile not found: {path}")]
    #[diagnostic(
        code(taskit::protocol::lockfile_missing),
        help("run `taskit check-protocol-drift --update` to generate")
    )]
    LockfileMissing { path: String },

    #[error("lockfile is stale")]
    #[diagnostic(
        code(taskit::protocol::stale),
        help("re-run `taskit check-protocol-drift --update`")
    )]
    Stale,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InitError {
    #[error("taskit.toml already exists")]
    #[diagnostic(code(taskit::init::exists), help("use --force to overwrite"))]
    AlreadyExists,

    #[error("cargo metadata failed: {reason}")]
    #[diagnostic(code(taskit::init::metadata))]
    CargoMetadata { reason: String },

    #[error("failed to write {file}: {reason}")]
    #[diagnostic(code(taskit::init::write))]
    WriteFile { file: String, reason: String },
}

#[derive(Debug, Error, Diagnostic)]
#[error("step \"{name}\" failed")]
#[diagnostic(severity(error))]
pub struct StepError {
    pub name: String,
    #[help]
    pub detail: Option<String>,
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
    fn config_invalid_display() {
        let err = ConfigError::Invalid {
            message: "missing workspace".into(),
            hint: "add [workspace] section".into(),
        };
        assert!(err.to_string().contains("missing workspace"));
    }
}
