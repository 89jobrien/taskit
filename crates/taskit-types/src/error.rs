use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum TaskitError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] ConfigError),

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
    fn config_invalid_display() {
        let err = ConfigError::Invalid {
            message: "missing workspace".into(),
            hint: "add [workspace] section".into(),
        };
        assert!(err.to_string().contains("missing workspace"));
    }
}
