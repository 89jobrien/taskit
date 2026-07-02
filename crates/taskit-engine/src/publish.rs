use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;
use crate::step::Pipeline;

/// Crates in publish order: dependencies before dependents.
const PUBLISH_ORDER: &[&str] = &[
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
    Ok(crate::output::write_output(ctx.output, &outcome)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_order_starts_with_core() {
        assert_eq!(PUBLISH_ORDER[0], "taskit-core");
    }

    #[test]
    fn publish_order_ends_with_root() {
        assert_eq!(*PUBLISH_ORDER.last().unwrap(), "taskit");
    }

    #[test]
    fn publish_order_has_all_crates() {
        assert_eq!(PUBLISH_ORDER.len(), 5);
        assert!(PUBLISH_ORDER.contains(&"taskit-core"));
        assert!(PUBLISH_ORDER.contains(&"taskit-engine"));
        assert!(PUBLISH_ORDER.contains(&"taskit-init"));
        assert!(PUBLISH_ORDER.contains(&"taskit-crux"));
        assert!(PUBLISH_ORDER.contains(&"taskit"));
    }

    #[test]
    fn core_before_engine() {
        let core_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-core")
            .unwrap();
        let engine_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-engine")
            .unwrap();
        assert!(core_pos < engine_pos);
    }

    #[test]
    fn core_before_init() {
        let core_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-core")
            .unwrap();
        let init_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-init")
            .unwrap();
        assert!(core_pos < init_pos);
    }

    #[test]
    fn engine_before_root() {
        let engine_pos = PUBLISH_ORDER
            .iter()
            .position(|&c| c == "taskit-engine")
            .unwrap();
        let root_pos = PUBLISH_ORDER.iter().position(|&c| c == "taskit").unwrap();
        assert!(engine_pos < root_pos);
    }
}
