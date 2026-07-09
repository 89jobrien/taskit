use crate::baml_client::types::ConflictInput;
use taskit_core::conflict_resolver::ConflictResolver;
use taskit_types::conflict::{ConflictFile, ResolvedFile};
use taskit_types::error::{FlowError, TaskitError};

/// LLM-assisted conflict resolver backed by the BAML `ResolveConflict` function.
pub struct BamlConflictResolver;

impl ConflictResolver for BamlConflictResolver {
    fn resolve(&self, files: &[ConflictFile]) -> Result<Vec<ResolvedFile>, TaskitError> {
        use crate::baml_client::sync_client::B;

        let inputs: Vec<ConflictInput> = files
            .iter()
            .map(|f| ConflictInput {
                path: f.path.clone(),
                ours: f.ours.clone(),
                theirs: f.theirs.clone(),
                base: f.base.clone(),
            })
            .collect();

        let resolution = B
            .ResolveConflict
            .call(&inputs)
            .map_err(|e| TaskitError::other(format!("BAML ResolveConflict failed: {e}")))?;

        // Escalate any paths the model flagged as needing human review.
        if let Some(path) = resolution.needs_human.into_iter().next() {
            return Err(FlowError::NeedsHuman {
                path,
                reason: "LLM confidence below threshold".into(),
            }
            .into());
        }

        Ok(resolution
            .resolved
            .into_iter()
            .map(|r| ResolvedFile::new(r.path, r.content))
            .collect())
    }
}
