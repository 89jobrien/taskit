use taskit_types::config::Config;

#[test]
fn parses_full_taskit_toml_fixture() {
    let cfg: Config = toml::from_str(include_str!("fixtures/full-taskit.toml"))
        .expect("full taskit TOML fixture should parse");

    assert_eq!(cfg.workspace.crates.len(), 2);
    assert_eq!(cfg.workspace.crates[0].pkg_name(), "taskit-types");
    assert_eq!(cfg.workspace.crates[1].pkg_name(), "custom-engine");
    assert_eq!(
        cfg.workspace.offline_skip_expr().as_deref(),
        Some("not test(network)")
    );

    let protocol = cfg.protocol.expect("protocol section");
    assert_eq!(protocol.lockfile_path(), "custom-protocol.lock");
    assert_eq!(protocol.surfaces.len(), 1);
    assert_eq!(protocol.surfaces[0].name, "core-api");

    let ci = cfg.ci.expect("ci section");
    assert_eq!(ci.steps.len(), 2);
    assert!(ci.steps[0].gate);
    assert!(!ci.steps[1].gate);

    let coverage = cfg.coverage.expect("coverage section");
    assert_eq!(coverage.crate_name, "taskit-engine");
    assert_eq!(coverage.threshold(), 87.5);

    let flow = cfg.flow.expect("flow section");
    assert_eq!(flow.main_branch(), "production");
    assert_eq!(flow.staging_branch(), "staging");
    assert_eq!(flow.release_branch(), "release");

    let release = cfg.release.expect("release section");
    assert_eq!(release.github_repo(), Some("89jobrien/taskit"));
    assert_eq!(release.publish_order, vec!["taskit-types", "taskit-engine"]);
}
