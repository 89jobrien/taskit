use crate::plan::InitPlan;

/// Render an InitPlan into taskit.toml content.
///
/// Sections that are configured get rendered as active TOML.
/// Sections that are available but not configured get rendered
/// as commented-out examples so users can see what's possible.
pub fn render_toml(plan: &InitPlan) -> String {
    let mut out = String::new();

    render_workspace(&mut out, plan);
    render_propagation(&mut out, plan);
    render_protocol(&mut out, plan);
    render_coverage(&mut out, plan);
    render_ci(&mut out, plan);
    render_flow(&mut out, plan);

    out
}

fn render_workspace(out: &mut String, plan: &InitPlan) {
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

    // offline_skip
    if let Some(ref expr) = plan.offline_skip {
        out.push_str(&format!("offline_skip = \"{}\"\n", expr));
    } else {
        out.push_str("# offline_skip = \"test(/.*network.*/)\"\n");
    }
}

fn render_propagation(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if plan.propagation.is_empty() {
        out.push_str(
            "\
# Propagation rules: when a source crate changes, its dependents are\n\
# automatically included in affected-crate detection.\n\
#\n\
# [[workspace.propagation]]\n\
# source = \"my-core\"\n\
# dependents = [\"my-engine\", \"my-cli\"]\n",
        );
    } else {
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
    }
}

fn render_protocol(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if plan.surfaces.is_empty() {
        out.push_str(
            "\
# Protocol drift detection: track SHA-256 hashes of contract surfaces.\n\
# Run `taskit check-protocol-drift --update` to regenerate the lockfile.\n\
#\n\
# [protocol]\n\
# lockfile = \"taskit-protocol.lock\"\n\
#\n\
# [[protocol.surfaces]]\n\
# name = \"core-api\"\n\
# path = \"crates/core/src/lib.rs\"\n",
        );
    } else {
        out.push_str("[protocol]\n");
        out.push_str("# lockfile = \"taskit-protocol.lock\"\n");
        for s in &plan.surfaces {
            out.push_str(&format!(
                "\n[[protocol.surfaces]]\nname = \"{}\"\npath = \"{}\"\n",
                s.name, s.path
            ));
        }
    }
}

fn render_coverage(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if let Some(ref cov) = plan.coverage {
        out.push_str(&format!(
            "[coverage]\ncrate_name = \"{}\"\nthreshold = {:.1}\n",
            cov.crate_name, cov.threshold
        ));
    } else {
        out.push_str(
            "\
# Coverage enforcement: run `taskit coverage` to check.\n\
#\n\
# [coverage]\n\
# crate_name = \"my-crate\"\n\
# threshold = 80.0\n",
        );
    }
}

fn render_ci(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if !plan.ci_steps.is_empty() {
        out.push_str("[ci]\n");
        out.push_str("# cruxfile = \"Cruxfile\"\n");
        for step in &plan.ci_steps {
            out.push_str(&format!(
                "\n[[ci.steps]]\nname = \"{}\"\ncmd = \"{}\"\ngate = {}\n",
                step.name, step.cmd, step.gate
            ));
        }
    } else {
        out.push_str(
            "\
# CI pipeline steps. Run `taskit ci` to execute all steps.\n\
#\n\
# [ci]\n\
# cruxfile = \"Cruxfile\"\n\
#\n\
# [[ci.steps]]\n\
# name = \"fmt --check\"\n\
# cmd = \"fmt --check\"\n\
# gate = false\n\
#\n\
# [[ci.steps]]\n\
# name = \"test\"\n\
# cmd = \"test\"\n\
# gate = false\n",
        );
    }
}

fn render_flow(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if let Some(ref flow) = plan.flow {
        let is_default =
            flow.main == "main" && flow.staging == "staging" && flow.release == "release";

        if is_default {
            out.push_str(
                "\
# Git flow: main (stable) -> staging (work) -> release (publish) -> main\n\
# These are the defaults; uncomment to customize branch names.\n\
#\n\
# [flow]\n\
# main = \"main\"\n\
# staging = \"staging\"\n\
# release = \"release\"\n",
            );
        } else {
            out.push_str("[flow]\n");
            out.push_str(&format!("main = \"{}\"\n", flow.main));
            out.push_str(&format!("staging = \"{}\"\n", flow.staging));
            out.push_str(&format!("release = \"{}\"\n", flow.release));
        }
    } else {
        out.push_str(
            "\
# Git flow: main (stable) -> staging (work) -> release (publish) -> main\n\
# Uncomment to enable `taskit flow` subcommands.\n\
#\n\
# [flow]\n\
# main = \"main\"\n\
# staging = \"staging\"\n\
# release = \"release\"\n",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CiStepPlan, CoveragePlan, CratePlan, FlowPlan, SurfacePlan};

    fn minimal_plan() -> InitPlan {
        InitPlan {
            crates: vec![CratePlan {
                dir: ".".into(),
                pkg: None,
            }],
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
        }
    }

    #[test]
    fn render_minimal_plan() {
        let toml = render_toml(&minimal_plan());
        assert!(toml.contains("[workspace]"));
        assert!(toml.contains("dir = \".\""));
    }

    #[test]
    fn render_commented_sections_when_empty() {
        let toml = render_toml(&minimal_plan());
        // All optional sections should appear as comments
        assert!(toml.contains("# [coverage]"));
        assert!(toml.contains("# [flow]"));
        assert!(toml.contains("# [protocol]"));
        assert!(toml.contains("# [[workspace.propagation]]"));
        assert!(toml.contains("# [[ci.steps]]"));
        assert!(toml.contains("# offline_skip"));
    }

    #[test]
    fn render_with_coverage() {
        let mut plan = minimal_plan();
        plan.coverage = Some(CoveragePlan {
            crate_name: "my-crate".into(),
            threshold: 80.0,
        });
        let toml = render_toml(&plan);
        assert!(toml.contains("[coverage]"));
        assert!(toml.contains("crate_name = \"my-crate\""));
        assert!(toml.contains("threshold = 80.0"));
    }

    #[test]
    fn render_with_ci_steps() {
        let mut plan = minimal_plan();
        plan.ci_steps = vec![CiStepPlan {
            name: "test".into(),
            cmd: "test".into(),
            gate: false,
        }];
        let toml = render_toml(&plan);
        assert!(toml.contains("[[ci.steps]]"));
        assert!(toml.contains("name = \"test\""));
    }

    #[test]
    fn render_with_surfaces() {
        let mut plan = minimal_plan();
        plan.surfaces = vec![SurfacePlan {
            name: "schema".into(),
            path: "schema.graphql".into(),
        }];
        let toml = render_toml(&plan);
        assert!(toml.contains("[protocol]"));
        assert!(toml.contains("[[protocol.surfaces]]"));
        assert!(toml.contains("name = \"schema\""));
    }

    #[test]
    fn render_with_propagation() {
        use taskit_types::config::PropagationEntry;
        let mut plan = minimal_plan();
        plan.propagation = vec![PropagationEntry {
            source: "common".into(),
            dependents: vec!["api".into(), "cli".into()],
        }];
        let toml = render_toml(&plan);
        assert!(toml.contains("[[workspace.propagation]]"));
        assert!(toml.contains("source = \"common\""));
        assert!(toml.contains("dependents = [\"api\", \"cli\"]"));
    }

    #[test]
    fn render_crate_with_pkg_remap() {
        let mut plan = minimal_plan();
        plan.crates = vec![CratePlan {
            dir: "crates/cli".into(),
            pkg: Some("my-cli".into()),
        }];
        let toml = render_toml(&plan);
        assert!(toml.contains("pkg = \"my-cli\""));
    }

    #[test]
    fn render_with_default_flow() {
        let mut plan = minimal_plan();
        plan.flow = Some(FlowPlan::default());
        let toml = render_toml(&plan);
        // Default flow should be commented since values match defaults
        assert!(toml.contains("# [flow]"));
        assert!(toml.contains("# main = \"main\""));
    }

    #[test]
    fn render_with_custom_flow() {
        let mut plan = minimal_plan();
        plan.flow = Some(FlowPlan {
            main: "trunk".into(),
            staging: "develop".into(),
            release: "prod".into(),
        });
        let toml = render_toml(&plan);
        assert!(toml.contains("[flow]"));
        assert!(toml.contains("main = \"trunk\""));
        assert!(toml.contains("staging = \"develop\""));
        assert!(toml.contains("release = \"prod\""));
    }

    #[test]
    fn render_with_offline_skip() {
        let mut plan = minimal_plan();
        plan.offline_skip = Some("test(/.*network.*/)".into());
        let toml = render_toml(&plan);
        assert!(toml.contains("offline_skip = \"test(/.*network.*/)\""));
        assert!(!toml.contains("# offline_skip"));
    }

    #[test]
    fn render_roundtrip_parses() {
        let mut plan = minimal_plan();
        plan.ci_steps = InitPlan::default_steps();
        plan.flow = Some(FlowPlan::default());
        let toml_str = render_toml(&plan);
        // The generated TOML should parse (ignoring comments)
        let parsed: Result<taskit_types::config::Config, _> = toml::from_str(&toml_str);
        assert!(
            parsed.is_ok(),
            "generated TOML should parse: {:?}",
            parsed.err()
        );
    }
}
