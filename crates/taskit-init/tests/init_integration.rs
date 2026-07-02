use taskit_init::plan::{self, InitPlan};
use taskit_init::render_cruxfile::render_cruxfile;
use taskit_init::render_toml::render_toml;

/// plan_from_discovery() -> render_toml() -> toml::from_str() must succeed.
#[test]
fn rendered_toml_parses_as_valid_config() {
    let plan = plan::plan_from_discovery().expect("plan_from_discovery failed");
    let toml_str = render_toml(&plan);
    toml::from_str::<taskit_types::config::Config>(&toml_str)
        .unwrap_or_else(|e| panic!("rendered TOML did not parse as Config: {e}\n---\n{toml_str}"));
}

/// render_cruxfile() output must be non-empty and contain "steps:".
#[test]
fn rendered_cruxfile_is_nonempty() {
    let plan = plan::plan_from_discovery().expect("plan_from_discovery failed");
    let crux = render_cruxfile(&plan, "test-project");
    assert!(!crux.is_empty(), "rendered Cruxfile was empty");
    assert!(
        crux.contains("steps:"),
        "rendered Cruxfile does not contain 'steps:'\n---\n{crux}"
    );
}

/// plan_from_discovery() must find at least 5 crates in the taskit workspace.
#[test]
fn plan_discovery_returns_all_workspace_crates() {
    let plan = plan::plan_from_discovery().expect("plan_from_discovery failed");
    assert!(
        plan.crates.len() >= 5,
        "expected at least 5 crates, found {}: {:?}",
        plan.crates.len(),
        plan.crates.iter().map(|c| &c.dir).collect::<Vec<_>>()
    );
}

/// InitPlan::default_steps() must return at least 5 steps.
#[test]
fn plan_default_steps_are_nonempty() {
    let steps = InitPlan::default_steps();
    assert!(
        steps.len() >= 5,
        "expected at least 5 default steps, got {}",
        steps.len()
    );
}

/// Rendered TOML must contain [workspace] and crates = [.
#[test]
fn rendered_toml_contains_workspace_section() {
    let plan = plan::plan_from_discovery().expect("plan_from_discovery failed");
    let toml_str = render_toml(&plan);
    assert!(
        toml_str.contains("[workspace]"),
        "rendered TOML missing [workspace] section\n---\n{toml_str}"
    );
    assert!(
        toml_str.contains("crates = ["),
        "rendered TOML missing 'crates = ['\n---\n{toml_str}"
    );
}
