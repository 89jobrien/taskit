use anyhow::Result;
use xshell::{Shell, cmd};

use crate::{config::WorkspaceConfig, progress::with_spinner, runner::xrun, util};

/// Nextest filter expression that excludes tests requiring external network access or
/// environment-specific credentials (GCS metadata server, macOS Keychain / ADC).
/// Applied by default in `cargo xtask ci`; override with `--include-network`.
pub const OFFLINE_SKIP_EXPR: &str = "not (\
    test(gcs::tests::test_serve_file_auth_failure) \
    | test(gcs::tests::test_get_access_token_invalid_json_response) \
    | test(test_resolve_agent_api_key_gemini)\
)";

pub fn run(
    sh: &Shell,
    ws: &WorkspaceConfig,
    crate_name: Option<&str>,
    use_affected: bool,
    continue_on_error: bool,
    offline: bool,
) -> Result<()> {
    let mut extra: Vec<String> = vec![
        "--status-level".into(),
        "none".into(),
        "--final-status-level".into(),
        "fail".into(),
        "--hide-progress-bar".into(),
    ];
    if continue_on_error {
        extra.push("--no-fail-fast".into());
    } else {
        extra.push("--fail-fast".into());
    }
    if offline {
        extra.extend(["-E".into(), OFFLINE_SKIP_EXPR.into()]);
    }
    let extra = extra.as_slice();
    util::run_per_crate(
        sh,
        ws,
        crate_name,
        use_affected,
        continue_on_error,
        |sh, name| {
            with_spinner(format!("test {name}"), || {
                xrun(cmd!(
                    sh,
                    "cargo nextest run --locked -p {name} --lib {extra...}"
                ))
            })
        },
        |sh| {
            with_spinner("test workspace", || {
                xrun(cmd!(
                    sh,
                    "cargo nextest run --locked --workspace --lib --exclude xtask {extra...}"
                ))
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- OFFLINE_SKIP_EXPR conformance ---

    #[test]
    fn offline_skip_expr_is_nonempty() {
        assert!(!OFFLINE_SKIP_EXPR.is_empty());
    }

    #[test]
    fn offline_skip_expr_starts_with_not() {
        // Must be a negation filter so tests not in the list are not excluded.
        assert!(
            OFFLINE_SKIP_EXPR.trim().starts_with("not"),
            "OFFLINE_SKIP_EXPR must start with 'not'"
        );
    }

    #[test]
    fn offline_skip_expr_contains_gcs_tests() {
        assert!(
            OFFLINE_SKIP_EXPR.contains("gcs::tests"),
            "GCS auth tests must be in offline skip list"
        );
    }

    #[test]
    fn offline_skip_expr_contains_gemini_test() {
        assert!(
            OFFLINE_SKIP_EXPR.contains("test_resolve_agent_api_key_gemini"),
            "gemini API key test must be in offline skip list"
        );
    }

    #[test]
    fn offline_skip_expr_uses_nextest_filter_syntax() {
        // Nextest -E expressions use `test(...)` predicate syntax.
        assert!(OFFLINE_SKIP_EXPR.contains("test("));
    }
}
