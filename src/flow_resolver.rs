use taskit_engine::flow::{ConflictFile, ConflictResolver, ResolvedFile};
use taskit_types::error::{FlowError, TaskitError};

/// LLM-assisted conflict resolver. Stub — BAML integration wired in fa7.
pub struct BamlConflictResolver;

impl ConflictResolver for BamlConflictResolver {
    fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
        // TODO(fa7): call BAML ResolveConflict function per file.
        // Escalate immediately until BAML is wired.
        let path = files
            .first()
            .map(|f| f.path.clone())
            .unwrap_or_else(|| "<unknown>".into());
        Err(FlowError::NeedsHuman {
            path,
            reason: "BAML conflict resolver not yet implemented".into(),
        }
        .into())
    }
}
