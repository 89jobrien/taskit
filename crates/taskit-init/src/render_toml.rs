use crate::plan::InitPlan;

/// Render an InitPlan into taskit.toml content (hand-built, no serde).
// TODO: add a round-trip test that parses output via toml::from_str::<Config>()
pub fn render_toml(plan: &InitPlan) -> String {
    let mut out = String::new();

    // [workspace]
    out.push_str("[workspace]\n");
    out.push_str("crates = [\n");
    for c in &plan.crates {
        if let Some(ref pkg) = c.pkg {
            out.push_str(&format!(
                "  {{ dir = \"{}\", pkg = \"{}\" }},\n",
                c.dir, pkg
            ));
        } else {
            out.push_str(&format!("  {{ dir = \"{}\" }},\n", c.dir));
        }
    }
    out.push_str("]\n");

    // [[workspace.propagation]]
    for p in &plan.propagation {
        out.push_str("\n[[workspace.propagation]]\n");
        out.push_str(&format!("source = \"{}\"\n", p.source));
        out.push_str(&format!(
            "dependents = [{}]\n",
            p.dependents
                .iter()
                .map(|t| format!("\"{}\"", t))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // [[workspace.surfaces]]
    for s in &plan.surfaces {
        out.push_str(&format!(
            "\n[[workspace.surfaces]]\nname = \"{}\"\npath = \"{}\"\n",
            s.name, s.path
        ));
    }

    // [coverage]
    if let Some(ref cov) = plan.coverage {
        out.push_str(&format!(
            "\n[coverage]\ncrate_name = \"{}\"\nthreshold = {:.1}\n",
            cov.crate_name, cov.threshold
        ));
    }

    // [ci]
    if !plan.ci_steps.is_empty() {
        out.push_str("\n[ci]\n");
        for step in &plan.ci_steps {
            out.push_str(&format!(
                "\n[[ci.steps]]\nname = \"{}\"\ncmd = \"{}\"\ngate = {}\n",
                step.name, step.cmd, step.gate
            ));
        }
    }

    // offline_skip
    if let Some(ref expr) = plan.offline_skip {
        out.push_str(&format!("\n[testing]\noffline_skip = \"{}\"\n", expr));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CiStepPlan, CoveragePlan, CratePlan, SurfacePlan};

    #[test]
    fn render_minimal_plan() {
        let plan = InitPlan {
            crates: vec![CratePlan {
                dir: ".".into(),
                pkg: None,
            }],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("[workspace]"));
        assert!(toml.contains("dir = \".\""));
    }

    #[test]
    fn render_with_coverage() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: Some(CoveragePlan {
                crate_name: "my-crate".into(),
                threshold: 80.0,
            }),
            ci_steps: vec![],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("[coverage]"));
        assert!(toml.contains("crate_name = \"my-crate\""));
        assert!(toml.contains("threshold = 80.0"));
    }

    #[test]
    fn render_with_ci_steps() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![CiStepPlan {
                name: "test".into(),
                cmd: "test".into(),
                gate: false,
            }],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("[[ci.steps]]"));
        assert!(toml.contains("name = \"test\""));
    }

    #[test]
    fn render_with_surfaces() {
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![],
            surfaces: vec![SurfacePlan {
                name: "schema".into(),
                path: "schema.graphql".into(),
            }],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("[[workspace.surfaces]]"));
        assert!(toml.contains("name = \"schema\""));
    }

    #[test]
    fn render_with_propagation() {
        use taskit_core::config::PropagationEntry;
        let plan = InitPlan {
            crates: vec![],
            propagation: vec![PropagationEntry {
                source: "common".into(),
                dependents: vec!["api".into(), "cli".into()],
            }],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("[[workspace.propagation]]"));
        assert!(toml.contains("source = \"common\""));
        assert!(toml.contains("dependents = [\"api\", \"cli\"]"));
    }

    #[test]
    fn render_crate_with_pkg_remap() {
        let plan = InitPlan {
            crates: vec![CratePlan {
                dir: "crates/cli".into(),
                pkg: Some("my-cli".into()),
            }],
            propagation: vec![],
            surfaces: vec![],
            coverage: None,
            ci_steps: vec![],
            offline_skip: None,
        };
        let toml = render_toml(&plan);
        assert!(toml.contains("pkg = \"my-cli\""));
    }
}
