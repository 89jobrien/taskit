# Idea: Gate Checkpoint / Skip-If-Passed System

## Effort

Quick Win (< 1 day)

## Source

Grounded in `minibox/xtask/src/checkpoint.rs` — a sibling project's dev-tool
xtask crate.

## Problem

`taskit ci` and `taskit quick` re-run every gate (fmt, lint, test, coverage,
protocol-drift, ...) on every invocation, even when nothing has changed since
the last green run for the exact same code state. For a fail-fast local loop
this is wasted wall-clock, especially on larger workspaces.

## Evidence

- `minibox/xtask/src/checkpoint.rs` implements a `GateId` + `CheckpointRecord`
  system that records which gates have already passed for the current code
  state, and skips re-running them.
- `taskit-engine/src/step.rs` — Pipeline builder (`step`/`step_with_context_sink`)
  has no equivalent skip-if-unchanged concept; every `Ci`/`Quick` run executes
  the full step list unconditionally.

## Proposed Direction

Add a checkpoint store (e.g. `target/taskit/checkpoints.json`, alongside the
existing `target/taskit/state.json` used for `flow auto` resumption) keyed by
a content hash of the relevant inputs per gate:
- `fmt`/`lint`: hash of tracked `.rs` files (or affected-crate subset).
- `test`: same, plus test file changes.
- `protocol-drift`: already has its own hash mechanism in `protocol/drift.rs`
  — could reuse directly.

Each `Pipeline::step` call would check the checkpoint before running, and
record success on completion. Invalidate on any source change under the
gate's tracked paths.

## Open Questions

- Should checkpoints be per-crate (aligning with `--affected` detection
  already in `command.rs`) or per-workspace?
- Does this conflict with `--fail-fast` semantics — should a skipped gate
  still count as "passed" in `PipelineOutcome`?
- Precedent for content-hash gating already exists in `protocol/drift.rs`
  (`calculate_lockfile`) — reuse that hashing approach for consistency.
