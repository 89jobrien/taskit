//! Adapts BAML's `ResolveConflict` function to taskit's conflict-resolver port.

use crate::baml_client::types::ConflictInput;
use taskit_core::conflict_resolver::ConflictResolver;
use taskit_types::conflict::{ConflictFile, ResolvedFile};
use taskit_types::error::{FlowError, TaskitError};

/// BAML client used when no remote provider credential is configured. Named
/// clients can be selected per call through BAML's client registry.
const LOCAL_CLIENT: &str = "Ollama_Llama";

/// Credential env vars for the remote clients in the `ConflictResolver`
/// fallback strategy, in chain order.
const REMOTE_CREDENTIAL_VARS: [&str; 2] = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY"];

/// LLM-assisted conflict resolver backed by the BAML `ResolveConflict` function.
///
/// The client chain lives in `baml_src/clients.baml`: Anthropic, then OpenAI,
/// then a local Ollama daemon. BAML advances past a provider whose call fails,
/// so a rejected credential falls through to Ollama. A credential that is not
/// set at all is a different failure — BAML cannot construct that client and
/// aborts the chain — so this resolver pins the call to the local client when
/// no remote provider is configured.
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

        let call = if remote_provider_configured(|var| std::env::var(var).ok()) {
            B.ResolveConflict.clone()
        } else {
            B.ResolveConflict.with_client(LOCAL_CLIENT)
        };
        let resolution = call
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

/// True when at least one remote provider has a usable, non-empty credential.
fn remote_provider_configured(lookup: impl Fn(&str) -> Option<String>) -> bool {
    REMOTE_CREDENTIAL_VARS
        .iter()
        .any(|var| lookup(var).is_some_and(|value| !value.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_credential_means_no_remote_provider() {
        assert!(!remote_provider_configured(|_| None));
    }

    #[test]
    fn blank_credential_does_not_count_as_configured() {
        assert!(!remote_provider_configured(|_| Some("   ".to_string())));
        assert!(!remote_provider_configured(|_| Some(String::new())));
    }

    #[test]
    fn any_non_empty_credential_enables_the_fallback_chain() {
        assert!(remote_provider_configured(|var| {
            (var == "OPENAI_API_KEY").then(|| "sk-live".to_string())
        }));
        assert!(remote_provider_configured(|var| {
            (var == "ANTHROPIC_API_KEY").then(|| "sk-live".to_string())
        }));
    }
}
