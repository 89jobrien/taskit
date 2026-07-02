// Output formatting is now owned by the taskit-output crate.
// Re-export everything for backwards compatibility.

pub use taskit_output::{
    DiagnosticFormatter, GithubFormatter, HumanFormatter, JsonFormatter, JunitFormatter,
    OutputFormatter, SarifFormatter, formatter_for, pipeline_error, write_output,
};

// Re-export OutputFormat from types.
pub use taskit_types::output_format::OutputFormat;
