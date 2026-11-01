//! Core ports and conformance helpers for taskit engine adapters.

/// Conflict-resolution port trait.
pub mod conflict_resolver;
/// Pipeline runner port trait.
pub mod pipeline_runner;
/// Step-construction helpers for pipeline assembly.
pub mod step_builder;

/// Re-export of the primary conflict-resolver port.
pub use conflict_resolver::ConflictResolver;

/// Shared conformance assertions for runner implementations.
pub mod conformance;
