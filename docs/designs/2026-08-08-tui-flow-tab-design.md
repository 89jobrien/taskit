# Design: TUI Flow Tab + Overview Protocol-Drift Line

## Goal

Add a Flow tab to `taskit-tui` showing git-flow pipeline position, resumable `flow auto`
state, the configured conflict resolver, and `flow auto` run telemetry; add a protocol-drift
status line to the existing Overview tab.

## Approved Approach

From brainstorming: new `Tab::Flow` in `taskit-tui` backed by new read-only data functions in
`taskit-engine` (`flow::status_report`, plus telemetry emitted by `flow::auto_with_ci`), and a
new `protocol::drift::check` function feeding one additional line in Overview's Health panel.
`cargo-deny` audit and CI→crux dispatch wiring are explicitly out of scope (see below).

## Crate Ownership

- **`taskit-engine`** — owns all new data-producing logic: `flow::status_report`,
  `flow::FlowStatusReport`/`FlowHop`, `flow` telemetry emission, `protocol::drift::check`,
  `protocol::drift::ProtocolDriftStatus`. No new external deps.
- **`taskit-tui`** — owns rendering and session state: `Tab::Flow`, `render_flow`, new
  `Snapshot` fields. Already depends on `taskit-engine`; no new inter-crate edges.
- **Affected crates**: `taskit-tui` imports the new `taskit-engine` types; no other crate
  consumes them.

## Public API

### `taskit-engine::flow` (additions)

```rust
// crates/taskit-engine/src/flow.rs

pub struct FlowHop {
    pub from: String,
    pub to: String,
    pub ahead: usize,
    pub behind: usize,
    pub branches_exist: bool,
}

pub struct FlowStatusReport {
    pub current_branch: String,
    /// main→develop→staging→release→main, in that order.
    pub hops: Vec<FlowHop>,
}

/// Pure data variant of `status()` — `status()` is refactored to call this and
/// print each hop, with no output change.
pub fn status_report(ctx: &Ctx, flow: &FlowConfig) -> Result<FlowStatusReport, TaskitError>;
```

### `taskit-engine::flow` (signature change)

```rust
// crates/taskit-engine/src/flow.rs

/// BREAKING: return type changes from `Result<(), TaskitError>` to
/// `Result<usize, TaskitError>` — the `usize` is the number of conflicted
/// files resolved (0 on the fast, no-conflict path). Needed so
/// `auto_with_ci` can accumulate a `flow_auto_conflicts` telemetry total.
pub fn merge_with_resolution(
    ctx: &Ctx,
    branch: &str,
    message: &str,
    resolver: &dyn ConflictResolver,
) -> Result<usize, TaskitError>;
```

### `taskit-engine::flow` (behavior change, no new signature)

`auto_with_ci` gains telemetry emission via the existing `crate::telemetry::record(ctx, &[...])`
(same call shape already used in `ci.rs`), at the two points where a run reaches a terminal
outcome for this invocation:

- On CI-gate failure (`FlowError::CiFailed` return path): record
  `flow_auto_duration_ms` (elapsed since entry), `flow_auto_result` (`0.0`),
  `flow_auto_conflicts` (accumulated count from the promote-phase merges).
- On full success (end of function, before `taskit_ok!`): record
  `flow_auto_duration_ms`, `flow_auto_result` (`1.0`), `flow_auto_conflicts`
  (total accumulated across all `merge_with_resolution` calls in the run).

No emission on a resumed-but-still-interrupted run (e.g. `NeedsHuman` escalation) — those
aren't terminal outcomes for the invocation and already have no equivalent in the existing
`ci_duration_ms`/`ci_passed` pattern this mirrors.

### `taskit-engine::protocol::drift` (additions)

```rust
// crates/taskit-engine/src/protocol/drift.rs

pub struct ProtocolDriftStatus {
    pub configured: bool,
    pub drifted_surfaces: Vec<String>,
}

/// Read-only: recomputes current surface hashes and compares against the
/// persisted lockfile. Never writes. Returns `configured: false` when no
/// `[[protocol.surfaces]]` are set (mirrors `run()`'s zero-config skip).
pub fn check(ctx: &Ctx) -> Result<ProtocolDriftStatus, TaskitError>;
```

### `taskit-tui` (additions)

```rust
// crates/taskit-tui/src/snapshot.rs (additions to Snapshot)
pub flow_status: Option<taskit_engine::flow::FlowStatusReport>,
pub flow_state: Option<taskit_types::flow_state::FlowState>,
pub flow_conflict_resolver: taskit_types::config::ConflictResolverKind,
pub flow_auto_duration_history: Vec<u64>,
pub flow_auto_result_history: Vec<u64>,
pub flow_auto_conflicts_last: Option<u64>,
pub protocol_drift: Option<taskit_engine::protocol::drift::ProtocolDriftStatus>,
```

```rust
// crates/taskit-tui/src/app.rs
pub enum Tab {
    Overview,
    Crates,
    History,
    Flow, // appended; Tab::ALL becomes a 4-element array
}
```

```rust
// crates/taskit-tui/src/ui.rs
fn render_flow(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot);
```

No new public functions beyond `render_flow` — everything else is data flowing through the
existing `render()` match and `Snapshot::collect`.

## Data Flow

1. **Flow hop table**: `Snapshot::collect` calls `flow::status_report(ctx, &ctx.config.flow.clone().unwrap_or_default())` → `Snapshot.flow_status` → `render_flow` renders a 4-row table.
2. **Resumable state**: `Snapshot::collect` calls `flow_state_store::load(ctx.root())` (already-existing function) → `Snapshot.flow_state` → `render_flow` shows a highlighted box with phase/hint/failed_steps when `Some`.
3. **Conflict resolver**: read directly from `ctx.config.flow` (no new function) → `Snapshot.flow_conflict_resolver` → one line in `render_flow`.
4. **Flow-auto telemetry**: `flow::auto_with_ci` writes `flow_auto_*` metrics via `telemetry::record` (existing sink) → `Snapshot::collect` filters `NdjsonStore::load_window` for those metric names (same pattern already used for `ci_duration_ms`/`ci_passed`) → `Snapshot.flow_auto_*_history` → a small trend strip in `render_flow`.
5. **Protocol drift line**: `Snapshot::collect` calls `protocol::drift::check(ctx)` → `Snapshot.protocol_drift` → `render_health` appends one line.

## Hexagonal Boundaries

Not applicable in the port/adapter sense — no new external I/O boundary is introduced. All
new functions read from the filesystem/git the same way existing `Snapshot::collect` code
already does (git subprocess for `status_report`'s ahead/behind counts, matching the existing
`is_clean`/`current_branch` pattern in `flow.rs`; plain file reads for telemetry and lockfile
comparison). No new trait is warranted — nothing here is swappable/mockable in a way the
codebase doesn't already handle via existing seams (`ConflictResolver`, `PipelineRunner`).

## Out of Scope

- `cargo-deny` audit signal on Overview — `audit::run` shells out live with no persisted
  result to read cheaply each tick; would need its own caching design first.
- Wiring `ci.cruxfile` → `SubprocessCruxRunner` into CI dispatch — currently zero live call
  sites anywhere in the codebase; a standalone feature, not a dashboard concern. Once it
  exists, `flow_auto` telemetry could fold in crux pass/fail as a later addition.
- Any new `[flow]` or `[protocol]` config fields — this design only reads existing config.

## Risk

- [ ] **Breaking API change: yes.** `merge_with_resolution` return type changes from
      `Result<(), TaskitError>` to `Result<usize, TaskitError>`. Call sites: 4 inside
      `flow.rs` (`auto_with_ci`, all already using `?` — need to capture the `usize` at the 3
      that participate in the conflict count, or continue discarding via `let _ = ... ?` at
      any that shouldn't count). Test call sites in
      `crates/taskit-engine/tests/flow_integration.rs` (4 tests) only assert `.is_ok()`/
      `.is_err()`/pattern-match on `Err` variants — none destructure `Ok(())` — so they
      compile unchanged.
- [ ] New external dependency: no.
- [ ] Feature flag required: no.
- [ ] Test coverage gaps to close in the plan (found, not fixed, during this design pass):
      - No test yet for `status_report`'s hop data (only the printing `status()` has an
        integration test, `flow_status_shows_all_branches`).
      - No test yet for `merge_with_resolution`'s returned conflict count (fast path = 0,
        conflict path = N).
      - No test yet asserting `flow_auto_*` telemetry is actually written by `auto_with_ci`
        (existing `flow_integration.rs` tests already spin up a temp git repo + `ctx.root`,
        so this is a `NdjsonStore::new(ctx.root()).load_window(..)` assertion after an
        `auto`/`auto_with_ci` call).
      - No test yet for `protocol::drift::check` (in-sync, drifted, and unconfigured cases).
      - `Snapshot::collect`'s existing inline tests (`from_parts`-based) will need extending
        for the new fields, and `app.rs`/`ui.rs` will need a `Tab::ALL` length update
        (currently hardcoded as `[Tab; 3]` — becomes `[Tab; 4]`).
