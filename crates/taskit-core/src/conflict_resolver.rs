use taskit_types::conflict::{ConflictFile, ResolvedFile};
use taskit_types::error::TaskitError;

/// Port for resolving merge conflicts — implemented by `BamlConflictResolver` in the binary.
pub trait ConflictResolver {
    fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError>;
}
