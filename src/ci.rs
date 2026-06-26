use anyhow::Result;
use xshell::Shell;

use crate::{
    DEFAULT_COVERAGE_THRESHOLD, check_deps,
    config::{ProtocolConfig, WorkspaceConfig},
    dev_setup, fmt, lint, protocol, schema,
    step::Pipeline,
    testing,
};

pub fn run(
    sh: &Shell,
    ws: &WorkspaceConfig,
    proto: Option<&ProtocolConfig>,
    fail_fast: bool,
    include_network: bool,
) -> Result<()> {
    let offline = !include_network;
    Pipeline::new(fail_fast)
        .gate("self-check", dev_setup::self_check)
        .step("fmt --check", || fmt::run(sh, ws, true, false))
        .step("lint", || lint::run(sh, ws, None, false, false))
        .step("compile-tests", || testing::compile::run(sh))
        .step("test", || {
            testing::run::run(sh, ws, None, false, false, offline)
        })
        .step("coverage (maestro-api)", || {
            testing::coverage::run(sh, "maestro-api", DEFAULT_COVERAGE_THRESHOLD)
        })
        .step("schema --check", || schema::run(sh, true))
        .step("check-deps", || check_deps::run(sh))
        .step("check-protocol-drift", || {
            let root = std::env::current_dir()?;
            protocol::drift::run(&root, proto, false, false, false)
        })
        .step("check-protocol-sites", || {
            protocol::sites::run(
                std::path::Path::new("maestro-common/src/session.rs"),
                "CreateSessionRequest {",
                4,
                false,
            )
        })
        .run()
}
