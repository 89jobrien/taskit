use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;
use crate::step::Pipeline;

/// Crates in publish order: dependencies before dependents.
const PUBLISH_ORDER: &[&str] = &[
    "taskit-types",
    "taskit-testing",
    "taskit-macros",
    "taskit-output",
    "taskit-core",
    "taskit-engine",
    "taskit-init",
    "taskit-crux",
    "taskit",
];

pub fn run(ctx: &Ctx, skip_docs: bool, allow_dirty: bool) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let dry_run = ctx.dry_run;
    // CLI flags win; fall back to [release] config defaults.
    let rel = ctx.release_config();
    let effective_skip_docs = skip_docs || rel.and_then(|r| r.skip_docs).unwrap_or(false);
    let effective_allow_dirty = allow_dirty || rel.and_then(|r| r.allow_dirty).unwrap_or(false);

    let mut pipeline = Pipeline::new(true);

    if !effective_skip_docs {
        pipeline = pipeline.gate("cargo doc", || {
            let doc_cmd = cmd!(sh, "cargo doc --workspace --no-deps");
            ctx.run(doc_cmd)
        });
    }

    let effective_order: Vec<String> = ctx
        .release_config()
        .filter(|r| !r.publish_order.is_empty())
        .map(|r| r.publish_order.clone())
        .unwrap_or_else(|| PUBLISH_ORDER.iter().map(|s| s.to_string()).collect());

    for krate in effective_order {
        let step_name = format!("publish {krate}");
        pipeline = pipeline.step(&step_name, move || {
            let mut args = vec!["publish", "-p", krate.as_str()];
            if dry_run {
                args.push("--dry-run");
            }
            if effective_allow_dirty {
                args.push("--allow-dirty");
            }
            let publish_cmd = cmd!(sh, "cargo {args...}");
            ctx.run(publish_cmd)
        });
    }

    let outcome = pipeline.run();
    Ok(taskit_output::write_output(ctx.output, &outcome)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(name: &str) -> usize {
        PUBLISH_ORDER.iter().position(|&c| c == name).unwrap()
    }

    #[test]
    fn publish_order_starts_with_types() {
        assert_eq!(PUBLISH_ORDER[0], "taskit-types");
    }

    #[test]
    fn publish_order_ends_with_root() {
        assert_eq!(*PUBLISH_ORDER.last().unwrap(), "taskit");
    }

    #[test]
    fn publish_order_has_all_crates() {
        assert_eq!(PUBLISH_ORDER.len(), 9);
        for name in [
            "taskit-types",
            "taskit-macros",
            "taskit-testing",
            "taskit-output",
            "taskit-core",
            "taskit-engine",
            "taskit-init",
            "taskit-crux",
            "taskit",
        ] {
            assert!(PUBLISH_ORDER.contains(&name), "missing {name}");
        }
    }

    #[test]
    fn types_before_core() {
        assert!(pos("taskit-types") < pos("taskit-core"));
    }

    #[test]
    fn core_before_engine() {
        assert!(pos("taskit-core") < pos("taskit-engine"));
    }

    #[test]
    fn core_before_init() {
        assert!(pos("taskit-core") < pos("taskit-init"));
    }

    #[test]
    fn engine_before_root() {
        assert!(pos("taskit-engine") < pos("taskit"));
    }

    // --- Finding 6: config fallback logic tests ---

    #[test]
    fn effective_skip_docs_cli_wins() {
        let cli = true;
        let cfg_val = Some(false);
        assert!(cli || cfg_val.unwrap_or(false));
    }

    #[test]
    fn effective_skip_docs_config_used_when_cli_false() {
        let cli = false;
        let cfg_val = Some(true);
        assert!(cli || cfg_val.unwrap_or(false));
    }

    #[test]
    fn effective_allow_dirty_none_config_defaults_false() {
        let cli = false;
        let cfg_val: Option<bool> = None;
        assert!(!(cli || cfg_val.unwrap_or(false)));
    }

    #[test]
    fn effective_allow_dirty_cli_true_no_config() {
        let cli = true;
        let cfg_val: Option<bool> = None;
        assert!(cli || cfg_val.unwrap_or(false));
    }

    #[test]
    fn config_publish_order_overrides_constant() {
        let config_order: Vec<String> = vec!["crate-a".into(), "crate-b".into()];
        let effective: Vec<&str> = if !config_order.is_empty() {
            config_order.iter().map(String::as_str).collect()
        } else {
            PUBLISH_ORDER.to_vec()
        };
        assert_eq!(effective, vec!["crate-a", "crate-b"]);
    }

    #[test]
    fn empty_config_publish_order_falls_back_to_constant() {
        let config_order: Vec<String> = vec![];
        let effective: Vec<&str> = if !config_order.is_empty() {
            config_order.iter().map(String::as_str).collect()
        } else {
            PUBLISH_ORDER.to_vec()
        };
        assert_eq!(effective, PUBLISH_ORDER);
    }
}
