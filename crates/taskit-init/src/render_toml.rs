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
    render_inspect(&mut out);
    render_clean(&mut out);
    render_flow(&mut out, plan);
    render_release(&mut out, plan);

    out
}

fn render_workspace(out: &mut String, plan: &InitPlan) {
    out.push_str("[workspace]\n");
    out.push_str("# root = \"/path/to/workspace\"  # defaults to Cargo.toml location\n");
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
# lockfile = \"taskit-protocol.lock\"  # default\n\
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
        out.push_str("# cruxfile  = \"Cruxfile\"  # path to Cruxfile for crux-based pipelines\n");
        out.push_str("# fail_fast = false        # stop on first failing step\n");
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
# cruxfile  = \"Cruxfile\"  # path to Cruxfile for crux-based pipelines\n\
# fail_fast = false        # stop on first failing step\n\
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

fn render_inspect(out: &mut String) {
    out.push('\n');
    out.push_str(
        "\
# Metric thresholds for `taskit inspect`. CLI flags override these.\n\
#\n\
# [inspect]\n\
# max_clippy_warnings = 0\n\
# max_clippy_errors   = 0\n\
# max_test_failures   = 0\n\
# max_todo_fixme      = 20   # omit to skip the TODO/FIXME check\n",
    );
}

fn render_clean(out: &mut String) {
    out.push('\n');
    out.push_str(
        "\
# Default retention policy for `taskit clean`. CLI --older-than overrides.\n\
#\n\
# [clean]\n\
# older_than = \"7d\"  # use `cargo sweep`; omit to run `cargo clean`\n",
    );
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

fn render_release(out: &mut String, plan: &InitPlan) {
    out.push('\n');
    if let Some(ref rel) = plan.release {
        out.push_str("[release]\n");
        if let Some(ref repo) = rel.github_repo {
            out.push_str(&format!("github_repo = \"{repo}\"\n"));
        } else {
            out.push_str("# github_repo = \"owner/repo\"  # auto-detected from git remote\n");
        }
        if !rel.publish_order.is_empty() {
            out.push_str("publish_order = [\n");
            for name in &rel.publish_order {
                out.push_str(&format!("  \"{name}\",\n"));
            }
            out.push_str("]\n");
        }
        out.push_str("# skip_docs   = false  # skip `cargo doc` before publishing\n");
        out.push_str("# allow_dirty = false  # publish with uncommitted changes\n");
    } else {
        out.push_str(
            "\
# Release configuration for `taskit publish` and `taskit release`.\n\
#\n\
# [release]\n\
# github_repo  = \"owner/repo\"\n\
# publish_order = [\"my-types\", \"my-core\", \"my-cli\"]\n\
# skip_docs    = false\n\
# allow_dirty  = false\n",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CiStepPlan, CoveragePlan, CratePlan, FlowPlan, ReleasePlan, SurfacePlan};

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
            release: None,
            git_hooks: false,
            github_ci: false,
            deny_toml: false,
            ctx_scaffold: false,
            mdbook: false,
            xtask: false,
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
        assert!(toml.contains("# fail_fast"));
        assert!(toml.contains("# offline_skip"));
        assert!(toml.contains("# [inspect]"));
        assert!(toml.contains("# [clean]"));
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
    fn render_commented_release_when_none() {
        let toml = render_toml(&minimal_plan());
        assert!(toml.contains("# [release]"));
    }

    #[test]
    fn render_with_release_config() {
        let mut plan = minimal_plan();
        plan.release = Some(ReleasePlan {
            github_repo: Some("89jobrien/my-project".into()),
            publish_order: vec!["my-types".into(), "my-core".into()],
        });
        let toml = render_toml(&plan);
        assert!(toml.contains("[release]"));
        assert!(toml.contains("github_repo = \"89jobrien/my-project\""));
        assert!(toml.contains("\"my-types\""));
        assert!(toml.contains("\"my-core\""));
    }

    #[test]
    fn render_release_without_repo() {
        let mut plan = minimal_plan();
        plan.release = Some(ReleasePlan {
            github_repo: None,
            publish_order: vec!["crate-a".into()],
        });
        let toml = render_toml(&plan);
        assert!(toml.contains("[release]"));
        assert!(toml.contains("# github_repo"));
        assert!(toml.contains("\"crate-a\""));
    }

    #[test]
    fn render_roundtrip_parses() {
        let mut plan = minimal_plan();
        plan.ci_steps = InitPlan::default_steps();
        plan.flow = Some(FlowPlan::default());
        let toml_str = render_toml(&plan);
        let parsed: Result<taskit_types::config::Config, _> = toml::from_str(&toml_str);
        assert!(
            parsed.is_ok(),
            "generated TOML should parse: {:?}",
            parsed.err()
        );
    }

    #[test]
    fn render_roundtrip_with_inspect_and_clean_parses() {
        // Active (uncommented) [inspect] and [clean] sections must round-trip through the parser.
        let toml_str = "\
[workspace]\ncrates = []\n\n\
[inspect]\nmax_clippy_warnings = 5\nmax_clippy_errors = 0\nmax_test_failures = 0\nmax_todo_fixme = 20\n\n\
[clean]\nolder_than = \"7d\"\n";
        let parsed: Result<taskit_types::config::Config, _> = toml::from_str(toml_str);
        assert!(
            parsed.is_ok(),
            "[inspect]/[clean] TOML should parse: {:?}",
            parsed.err()
        );
        let cfg = parsed.unwrap();
        let inspect = cfg.inspect.expect("[inspect] should be present");
        assert_eq!(inspect.max_clippy_warnings, Some(5));
        assert_eq!(inspect.max_todo_fixme, Some(20));
        let clean = cfg.clean.expect("[clean] should be present");
        assert_eq!(clean.older_than.as_deref(), Some("7d"));
    }
}
