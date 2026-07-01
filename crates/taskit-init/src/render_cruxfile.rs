use crate::escape::{comment_text, yaml_string};
use crate::plan::InitPlan;

/// Render an InitPlan into a Cruxfile (YAML pipeline for crux).
///
/// All interpolated values are emitted as double-quoted YAML scalars: the
/// generated `run:` lines are executed by `crux`, so an unescaped step name
/// or command could otherwise inject arbitrary pipeline steps.
pub fn render_cruxfile(plan: &InitPlan, project_name: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("# Cruxfile for {}\n", comment_text(project_name)));
    out.push_str(&format!(
        "name: {}\n",
        yaml_string(&format!("{project_name}-ci"))
    ));
    out.push_str("steps:\n");

    if plan.ci_steps.is_empty() {
        // Fallback: single taskit ci step
        out.push_str("  - name: ci\n");
        out.push_str("    run: taskit ci\n");
    } else {
        for step in &plan.ci_steps {
            out.push_str(&format!("  - name: {}\n", yaml_string(&step.name)));
            out.push_str(&format!(
                "    run: {}\n",
                yaml_string(&format!("taskit {}", step.cmd))
            ));
            if step.gate {
                out.push_str("    gate: true\n");
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::CiStepPlan;

    #[test]
    fn render_cruxfile_with_steps() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![
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
            ],
            offline_skip: None,
            flow: None,
            git_hooks: false,
            github_ci: false,
            deny_toml: false,
            ctx_scaffold: false,
            mdbook: false,
        };
        let yaml = render_cruxfile(&plan, "myproject");
        assert!(yaml.contains("name: \"myproject-ci\""));
        assert!(yaml.contains("name: \"fmt --check\""));
        assert!(yaml.contains("run: \"taskit fmt --check\""));
        assert!(yaml.contains("gate: true"));
    }

    #[test]
    fn render_cruxfile_escapes_hostile_step_name() {
        let plan = InitPlan {
            ci_steps: vec![CiStepPlan {
                name: "x\nsteps:\n  - name: pwn\n    run: evil".into(),
                cmd: "test".into(),
                gate: false,
            }],
            ..InitPlan::default()
        };
        let yaml = render_cruxfile(&plan, "proj");
        // The hostile name must stay inside one quoted scalar: no rendered
        // line may begin a new `run:` entry from the injected content.
        assert!(
            !yaml
                .lines()
                .any(|l| l.trim_start().starts_with("run: evil")),
            "newlines in step names must not inject YAML: {yaml}"
        );
    }

    #[test]
    fn render_cruxfile_empty_steps_fallback() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
            flow: None,
            git_hooks: false,
            github_ci: false,
            deny_toml: false,
            ctx_scaffold: false,
            mdbook: false,
        };
        let yaml = render_cruxfile(&plan, "proj");
        assert!(yaml.contains("run: taskit ci"));
    }

    #[test]
    fn render_cruxfile_contains_project_name() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
            flow: None,
            git_hooks: false,
            github_ci: false,
            deny_toml: false,
            ctx_scaffold: false,
            mdbook: false,
        };
        let yaml = render_cruxfile(&plan, "taskit");
        assert!(yaml.contains("# Cruxfile for taskit"));
        assert!(yaml.contains("name: \"taskit-ci\""));
    }
}
