# HANDOFF.local.md

## Session: 2026-07-01 — taskit SOLID refactor (Command port + Ctx)

### Status: complete — CI green (488 tests, clippy -D warnings, drift, machete all pass)

### Goal

Implement code-review finding #2 (introduce a `Command` port with an injected
`Ctx` execution context), then address the remaining review findings (#3 global
state, #4 wide signatures, #5 output shim). Plus fix the reported
`crux check Cruxfile` failure.

### Task list

| #   | Item                                                                        | Status                |
| --- | --------------------------------------------------------------------------- | --------------------- |
| 2   | `Command` trait + `Ctx` struct; main.rs dispatches via `Box<dyn Command>`   | DONE                  |
| 3   | Remove process-global `runner::DRY_RUN`/`SILENT`; inject via `Ctx`          | DONE                  |
| 4   | Collapse wide command signatures (config/shell/flags now on `Ctx`)          | DONE                  |
| 5   | Delete `engine/output.rs` re-export shim; use `taskit_output` directly      | DONE                  |
| —   | Fix `crux check Cruxfile` (generator emitted taskit-native, not crux, YAML) | DONE                  |
| 1   | Split the 8.4k-LOC `taskit-engine` god crate                                | DEFERRED (plan below) |

### CruxCtx decision

Requested "use CruxCtx if possible" — NOT used, and not possible without harm:
`taskit-crux` is a zero-dependency stub that does not depend on the real `crux`
crate, so `CruxCtx` is unavailable in this workspace; and `CruxCtx` is an
agentic-pipeline trace context, not a CLI execution context — wiring it in would
invert the optional `crux` feature dependency. Built a native `taskit-engine::Ctx`.

### What changed

- NEW `crates/taskit-engine/src/ctx.rs` — `Ctx { sh, root, config, dry_run,
output, silent }` with config accessors (`ws/proto/cov/ci/flow/root`) and
  execution methods (`run/run_capture/run_ok/with_silent`). Owns what were
  process globals; `silent` is a `Cell<bool>` (single-threaded CLI).
- NEW `crates/taskit-engine/src/command.rs` — `trait Command { fn run(&self,
ctx: &Ctx) }` plus one struct per subcommand (thin adapters over the module
  `run` fns). New auto-detected `pub trait` protocol surface `taskit-engine-command`
  (now tracked in `taskit.toml` + `taskit-protocol.lock`, 7 surfaces).
- DELETED `crates/taskit-engine/src/runner.rs` (globals gone) and
  `crates/taskit-engine/src/output.rs` (shim gone).
- `src/main.rs` — builds one `Ctx`, maps `Cmd` → `Box<dyn Command>` in
  `to_command`, dispatches. No more per-arm bespoke wiring.
- Threaded `&Ctx` through ~23 modules (fmt, lint, testing/\*, ci, quick, hooks,
  flow, dev_setup, health, publish, inspect, protocol/drift, audit, check_deps,
  check_freshness, clean, version, update_claude). Pure shell helpers
  (`util::run_per_crate`, `affected::detect`, `progress::with_spinner`, flow git
  readers, hooks hashers, health collectors) still take `&Shell` (`&ctx.sh`).
- `pipeline_runner.rs` `BuiltinRunner` now holds `{ ctx, offline }` instead of 6
  separate refs; reads config from `ctx`.
- `protocol/drift::run(ctx, update, warn_only, hook)` — root/proto/dry_run from ctx.
- `crates/taskit-init/src/render_cruxfile.rs` — now emits valid crux-script:
  `pipeline:` + `steps: [{ step, handler: shell::exec, args: { cmd } }]`
  (`shell::exec` fails the pipeline on non-zero exit, matching gate semantics).
  Step names slugified to identifiers. Root `Cruxfile` regenerated;
  `crux check Cruxfile` → ok (7 steps).

### Deferred: #1 god-crate split (plan)

`taskit-engine` is ~8.4k LOC / ~27 modules with unrelated reasons to change.
Suggested split (own effort, large mechanical diff):

- `taskit-quality` — fmt, lint, audit, inspect, health
- `taskit-vcs` — flow, hooks
- keep `taskit-testing` (already partial), pipeline/step/ctx/command in engine core
  Not bundled here to keep this diff reviewable; no behavior depends on it.

### Verify

- `cargo run -p taskit -- ci` → all 7 steps PASS
- `crux check Cruxfile` → ok
- binary reinstalled via `cargo install --path .`

### Notes

- `docs/plans/2026-06-27-*.md` still references `taskit_engine::output::write_output`
  — historical planning doc, intentionally left as a record.

---

## Session: 2026-07-01 — orca-strait plugin (unrelated, paused)

### Status: paused

### Done

- Diagnosed orca-strait plugin load error (`/doctor` showed
  `hooks` schema mismatch)
- Root cause: Claude Code 2.1.74 expects a different hooks record
  schema than what orca-strait provides; cached bazaar copy at
  `~/.claude/plugins/cache/bazaar/orca-strait/b7d1ec6/` has the
  same plugin.json
- Temporarily emptied `hooks` in
  `~/.claude/plugins/orca-strait/.claude-plugin/plugin.json`
  (set to `{}`)

### Next

- Update Claude Code (`brew upgrade claude-code`) from 2.1.74 to
  latest (stable 2.1.185+)
- After upgrade, restore orca-strait hooks in plugin.json and
  re-test with `/reload-plugins` + `/doctor`
- Also update the bazaar-cached copy if needed:
  `~/.claude/plugins/cache/bazaar/orca-strait/b7d1ec6/.claude-plugin/plugin.json`

### Notes

- The local copy at `~/.claude/plugins/orca-strait/` and the
  bazaar cache copy have identical plugin.json content
- The hook itself (`check-blocked.sh`) exists and is fine; only
  the schema wrapper is incompatible with the older CLI version
