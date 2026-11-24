//! Shared domain types used across the taskit workspace.
//!
//! `unused_assignments` is allowed crate-wide because `miette`'s `Diagnostic`
//! derive emits match arms (`help`, `labels`, `source_code`) that bind every
//! variant field, so any field a given arm ignores is reported at its
//! declaration. The lint cannot be scoped to the enum: the generated arms are
//! not inside it.
#![allow(unused_assignments)]

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
