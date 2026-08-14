# Idea: TUI Health Tab

## Effort

Medium (1-3 days)

## Problem

`taskit health` is a top-level CLI command backed by `crates/taskit-engine/src/health.rs`
(932 lines — the largest single-purpose module in taskit-engine after `formatter.rs`),
covering seven metrics (test count, clippy warnings, TODO density, public API surface,
dependency count, module size, doc coverage) plus baseline comparison and trend
tracking. Yet `taskit-tui` has no dedicated Health tab — only Overview, Crates,
History, and Flow. A user watching the TUI has no live view into health trend data
that the CLI already computes.

## Evidence

- `crates/taskit-tui/src/app.rs`: `Tab::ALL` = `[Overview, Crates, History, Flow]`.
- `crates/taskit-engine/src/health.rs` (932 lines) implements `health::run(ctx,
  update, with_coverage)` with `--with-coverage` just added this session.
- Overview tab already renders a "CI Telemetry & Drift (7d)" panel
  (`ui.rs:175`) showing `ci_duration_drift`/`protocol_drift` — precedent for
  pulling engine metrics into TUI panels exists (see recent Flow tab work,
  commits `e6f4580`/`6bbb510`/`3914e56`).

## Proposed Direction

Follow the same pattern used for the recently-added `Tab::Flow`
(`docs/designs/2026-08-08-tui-flow-tab-design.md`):

1. Add a read-only `health::snapshot_report` (or reuse existing health output
   struct) in `taskit-engine` that the TUI can call without side effects.
2. Add `Tab::Health` + `Snapshot` fields in `taskit-tui`.
3. Render the seven metrics plus trend vs `.health-baseline.json`, similar to how
   Flow renders `flow_auto_*` telemetry.

## Open Questions

- Should `--with-coverage` (expensive, instrumented build) ever run automatically
  from the TUI, or should the Health tab only show the last computed value and
  prompt the user to run `taskit health --with-coverage` manually?
- Fold Coverage/Audit visibility (see companion idea doc) into this same tab, or
  keep them separate?
