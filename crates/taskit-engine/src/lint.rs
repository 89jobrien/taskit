use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

use crate::{config::WorkspaceConfig, progress::with_spinner, runner::xrun, util};

pub fn run(
    sh: &Shell,
    ws: &WorkspaceConfig,
    crate_name: Option<&str>,
    use_affected: bool,
    continue_on_error: bool,
) -> Result<(), TaskitError> {
    util::run_per_crate(
        sh,
        ws,
        crate_name,
        use_affected,
        continue_on_error,
        |sh, name| {
            with_spinner(format!("lint {name}"), || {
                xrun(cmd!(
                    sh,
                    "cargo clippy --locked --quiet -p {name} --all-targets -- -D warnings"
                ))
            })
        },
        |sh| {
            with_spinner("lint workspace", || {
                xrun(cmd!(
                    sh,
                    "cargo clippy --locked --quiet --all-targets --workspace --exclude xtask -- -D warnings"
                ))
            })
        },
    )
}
