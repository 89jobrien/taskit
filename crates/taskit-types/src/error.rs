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
    fn config_invalid_display() {
        let err = ConfigError::Invalid {
            message: "missing workspace".into(),
            hint: "add [workspace] section".into(),
        };
        assert!(err.to_string().contains("missing workspace"));
    }
}
