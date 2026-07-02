use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;
use crate::step::Pipeline;

/// Crates in publish order: dependencies before dependents.
const PUBLISH_ORDER: &[&str] = &[
    "taskit-types",
    "taskit-macros",
    "taskit-testing",
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
    let mut pipeline = Pipeline::new(true);

    if !skip_docs {
        pipeline = pipeline.gate("cargo doc", || {
            let doc_cmd = cmd!(sh, "cargo doc --workspace --no-deps");
            ctx.run(doc_cmd)
        });
    }

    for krate in PUBLISH_ORDER {
        let krate = *krate;
        let step_name = format!("publish {krate}");
        pipeline = pipeline.step(&step_name, move || {
            let mut args = vec!["publish", "-p", krate];
            if dry_run {
                args.push("--dry-run");
            }
            if allow_dirty {
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
}
