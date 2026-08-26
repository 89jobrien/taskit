//! Engine crate containing task orchestration and command execution logic.

/// Affected-crate detection and propagation.
pub mod affected;
/// Cargo-deny audit orchestration.
pub mod audit;
/// Repository bootstrap command implementation.
pub mod bootstrap;
/// Build command implementation.
pub mod build;
/// Execution caching helpers.
pub mod cache;
/// Changelog generation through git-cliff.
pub mod changelog;
/// Dependency check command implementation.
pub mod check_deps;
/// Dependency freshness check implementation.
pub mod check_freshness;
/// CI pipeline orchestration.
pub mod ci;
/// Artifact cleanup command implementation.
pub mod clean;
/// CLI command model and dispatch primitives.
pub mod command;
/// Config loading and adaptation helpers.
pub mod config;
/// Shared runtime command context.
pub mod ctx;
/// Developer setup command implementation.
pub mod dev_setup;
/// Workspace discovery helpers.
pub mod discovery;
/// Health drift measurement.
pub mod drift;
/// Branch flow automation.
pub mod flow;
/// Flow-state persistence helpers.
pub mod flow_state_store;
/// Formatting command implementation.
pub mod fmt;
/// Health baseline and metrics commands.
pub mod health;
/// Git hook integration helpers.
pub mod hooks;
/// Health inspection command implementation.
pub mod inspect;
/// Install command implementation.
pub mod install;
/// Lint command implementation.
pub mod lint;
/// Semantic-version release helpers.
pub mod patch;
/// Pipeline runner adapters.
pub mod pipeline_runner;
/// Progress reporting helpers.
pub mod progress;
/// Protocol-related commands and utilities.
pub mod protocol;
/// Publish command implementation.
pub mod publish;
/// Quick-check command implementation.
pub mod quick;
/// Release command implementations.
pub mod release;
/// Step execution primitives.
pub mod step;
/// CI telemetry persistence and querying.
pub mod telemetry;
/// Test command implementations.
pub mod testing;
/// TODO sync command implementation.
pub mod todo_sync;
/// Dependency update command implementation.
pub mod update;
/// Claude version update command implementation.
pub mod update_claude;
/// Internal utility helpers.
pub mod util;
/// Version reporting command implementation.
pub mod version;

/// Re-export of the top-level command trait.
pub use command::Command;
/// Re-export of the engine runtime context.
pub use ctx::Ctx;

/// Resolved workspace root and parsed config.
#[derive(Debug)]
pub struct Workspace {
    /// Resolved workspace root directory.
    pub root: std::path::PathBuf,
    /// Parsed taskit configuration.
    pub config: taskit_types::config::Config,
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_crate_compiles() {
        // Verify the crate's public surface is accessible
        let _ws = super::Workspace {
            root: std::path::PathBuf::from("."),
            config: taskit_types::config::Config::default(),
        };
    }
}
