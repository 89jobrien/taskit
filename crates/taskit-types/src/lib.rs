//! Shared domain types used across the taskit workspace.

/// Runtime and file-backed configuration structures.
pub mod config;
/// Merge-conflict transport objects used by flow conflict resolution.
pub mod conflict;
/// Typed error hierarchy for taskit commands and adapters.
pub mod error;
/// Persisted state model for resumable `flow auto` operations.
pub mod flow_state;
/// Supported output rendering formats for CLI/reporting.
pub mod output_format;
/// Pipeline execution result and diagnostic data structures.
pub mod step;

/// Re-export of conflict request/response file types.
pub use conflict::{ConflictFile, ResolvedFile};
