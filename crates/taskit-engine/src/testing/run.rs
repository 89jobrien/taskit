use anyhow::Result;
use xshell::{Shell, cmd};

use crate::{config::WorkspaceConfig, progress::with_spinner, runner::xrun, util};

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
    if offline && let Some(expr) = ws.offline_skip_expr() {
        extra.extend(["-E".into(), expr]);
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
                    "cargo nextest run --locked -p {name} --all-targets {extra...}"
                ))
            })
        },
        |sh| {
            with_spinner("test workspace", || {
                xrun(cmd!(
                    sh,
                    "cargo nextest run --locked --workspace --all-targets {extra...}"
                ))
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::config::WorkspaceConfig;

    #[test]
    fn offline_skip_expr_returns_none_by_default() {
        let ws = WorkspaceConfig::default();
        assert!(ws.offline_skip_expr().is_none());
    }

    #[test]
    fn offline_skip_expr_returns_configured_value() {
        let ws = WorkspaceConfig {
            offline_skip: Some("not test(slow)".into()),
            ..Default::default()
        };
        assert_eq!(ws.offline_skip_expr().as_deref(), Some("not test(slow)"));
    }
}
