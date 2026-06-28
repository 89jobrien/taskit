use crate::plan::InitPlan;

/// Render an InitPlan into a Cruxfile (YAML pipeline for crux).
pub fn render_cruxfile(plan: &InitPlan, project_name: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("# Cruxfile for {}\n", project_name));
    out.push_str(&format!("name: {}-ci\n", project_name));
    out.push_str("steps:\n");

    if plan.ci_steps.is_empty() {
        // Fallback: single taskit ci step
        out.push_str("  - name: ci\n");
        out.push_str("    run: taskit ci\n");
    } else {
        for step in &plan.ci_steps {
            // TODO: quote YAML values containing spaces (e.g. "fmt --check")
            out.push_str(&format!("  - name: {}\n", step.name));
            out.push_str(&format!("    run: taskit {}\n", step.cmd));
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
        };
        let yaml = render_cruxfile(&plan, "myproject");
        assert!(yaml.contains("name: myproject-ci"));
        assert!(yaml.contains("name: fmt --check"));
        assert!(yaml.contains("run: taskit fmt --check"));
        assert!(yaml.contains("gate: true"));
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
        };
        let yaml = render_cruxfile(&plan, "taskit");
        assert!(yaml.contains("# Cruxfile for taskit"));
        assert!(yaml.contains("name: taskit-ci"));
    }
}
