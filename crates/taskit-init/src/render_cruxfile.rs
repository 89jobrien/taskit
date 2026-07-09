use crate::plan::InitPlan;

/// Render an InitPlan into a Cruxfile (a crux-script YAML pipeline).
///
/// Each taskit CI step becomes a `shell::exec` step (which fails the pipeline
/// on non-zero exit, matching taskit's gating semantics). The schema follows
/// crux-script: a top-level `pipeline:` name and a `steps:` list of
/// `{ step, handler, args }` nodes.
pub fn render_cruxfile(plan: &InitPlan, project_name: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("# Cruxfile for {project_name}\n"));
    out.push_str(&format!("pipeline: {project_name}-ci\n"));
    out.push_str("steps:\n");

    if plan.ci_steps.is_empty() {
        // Fallback: single `taskit ci` step.
        push_step(&mut out, "ci", "taskit ci");
    } else {
        for step in &plan.ci_steps {
            push_step(
                &mut out,
                &step_id(&step.name),
                &format!("taskit {}", step.cmd),
            );
        }
    }

    out
}

/// Emit a single `shell::exec` step node.
fn push_step(out: &mut String, id: &str, cmd: &str) {
    out.push_str(&format!("  - step: {id}\n"));
    out.push_str("    handler: shell::exec\n");
    out.push_str("    args:\n");
    out.push_str(&format!("      cmd: \"{}\"\n", cmd.replace('"', "\\\"")));
}

/// Turn a human step name (e.g. `"fmt --check"`) into a crux step identifier
/// (`"fmt_check"`): lowercase, non-alphanumeric runs collapsed to `_`.
fn step_id(name: &str) -> String {
    let mut id = String::with_capacity(name.len());
    let mut prev_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore {
            id.push('_');
            prev_underscore = true;
        }
    }
    id.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::CiStepPlan;

    fn plan_with(ci_steps: Vec<CiStepPlan>) -> InitPlan {
        InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps,
            offline_skip: None,
            flow: None,
            release: None,
            git_hooks: false,
            github_ci: false,
            deny_toml: false,
            ctx_scaffold: false,
            mdbook: false,
        }
    }

    #[test]
    fn render_cruxfile_uses_crux_pipeline_schema() {
        let plan = plan_with(vec![
            CiStepPlan {
                name: "fmt --check".into(),
                cmd: "fmt --check".into(),
                gate: false,
            },
            CiStepPlan {
                name: "test".into(),
                cmd: "test".into(),
                gate: true,
            },
        ]);
        let yaml = render_cruxfile(&plan, "myproject");
        assert!(yaml.contains("pipeline: myproject-ci"));
        // Step ids are slugified; commands live under args.cmd.
        assert!(yaml.contains("- step: fmt_check"));
        assert!(yaml.contains("handler: shell::exec"));
        assert!(yaml.contains("cmd: \"taskit fmt --check\""));
        assert!(yaml.contains("- step: test"));
        assert!(yaml.contains("cmd: \"taskit test\""));
        // The legacy taskit-native keys must not appear.
        assert!(!yaml.contains("run:"));
        assert!(!yaml.contains("gate:"));
    }

    #[test]
    fn render_cruxfile_empty_steps_fallback() {
        let plan = plan_with(vec![]);
        let yaml = render_cruxfile(&plan, "proj");
        assert!(yaml.contains("pipeline: proj-ci"));
        assert!(yaml.contains("- step: ci"));
        assert!(yaml.contains("cmd: \"taskit ci\""));
    }

    #[test]
    fn render_cruxfile_contains_project_name() {
        let plan = plan_with(vec![]);
        let yaml = render_cruxfile(&plan, "taskit");
        assert!(yaml.contains("# Cruxfile for taskit"));
        assert!(yaml.contains("pipeline: taskit-ci"));
    }

    #[test]
    fn step_id_slugifies_names() {
        assert_eq!(step_id("fmt --check"), "fmt_check");
        assert_eq!(step_id("check-protocol-drift"), "check_protocol_drift");
        assert_eq!(step_id("test"), "test");
        assert_eq!(step_id("coverage (my-crate)"), "coverage_my_crate");
    }
}
