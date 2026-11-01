# Design: Informative Dashboard Overview

## Goal

Turn the dashboard's Overview tab into an informative fastfetch-style project home page that
summarizes project identity, portable development-system facts, agentic context, workspace
composition, configured CI, health, and current activity.

## Approved Approach

Use the approved "Project summary" approach with a fastfetch-style header, portable development
stats, and an Agentic Context panel containing both repository signals and compact task counts.
The page is assembled from data taskit already collects, has in `Ctx`, or can read once from
well-known project files without adding a system-information dependency.

## Context Map

### Files to Modify

| File                           | Purpose                    | Changes Needed                                                                            |
| ------------------------------ | -------------------------- | ----------------------------------------------------------------------------------------- |
| `crates/taskit-tui/src/app.rs` | Startup/session state      | Capture project, system, CI, and agentic context once at startup.                         |
| `crates/taskit-tui/src/ui.rs`  | Ratatui layout and widgets | Render Project, Workspace, Pipeline, Health, and Activity panels; add buffer-based tests. |

### Dependencies

| File                                | Relationship                                                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `crates/taskit-engine/src/ctx.rs`   | Existing source for root, taskit version, commit SHA, Rust/Cargo versions, workspace members, and CI config. |
| `crates/taskit-tui/src/snapshot.rs` | Existing live source for branch, health baseline, protocol drift, CI, and flow state.                        |
| `crates/taskit-tui/src/lib.rs`      | Calls `App::new` and passes `App` plus `Snapshot` to the renderer; no change expected.                       |

### Test Coverage

| Test Location                       | Covers                                                                                                         |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `crates/taskit-tui/src/app.rs`      | Existing tab and scroll behavior; add unit tests for task-state parsing and project-file discovery.            |
| `crates/taskit-tui/src/snapshot.rs` | Existing health, telemetry, flow, and protocol snapshot derivation.                                            |
| `crates/taskit-tui/src/ui.rs`       | No current render tests; add a `TestBackend` assertion for populated overview content and constrained layouts. |

### Reference Patterns

| File                           | Pattern to Follow                                                         |
| ------------------------------ | ------------------------------------------------------------------------- |
| `crates/taskit-tui/src/ui.rs`  | Existing panel blocks, status colors, and responsive Ratatui constraints. |
| `crates/taskit-tui/src/app.rs` | Existing one-time `pipeline_run_context` workspace metadata collection.   |

### Risk

- `App` is public, but the new summary data remains crate-private and does not change existing fields.
- `Snapshot` and engine APIs remain unchanged, avoiding cross-crate and serialized-data changes.
- Agent task counts are best-effort summaries of common YAML `status:` values; unknown schemas remain visible as detected files without fabricated counts.
- Small terminals must retain useful content without panics or overlapping fixed-height panels.

## Crate Ownership

- **Owner crate**: `taskit-tui` - this is presentation-specific state and rendering.
- **Affected crates**: none; `taskit-tui` already depends on `taskit-engine` and `taskit-types`.

## Public API

No new public API is required. Add crate-private startup models:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectInfo {
    pub(crate) name: String,
    pub(crate) root: String,
    pub(crate) taskit_version: String,
    pub(crate) git_sha: Option<String>,
    pub(crate) rustc_version: Option<String>,
    pub(crate) cargo_version: Option<String>,
    pub(crate) os: &'static str,
    pub(crate) arch: &'static str,
    pub(crate) logical_cpus: usize,
    pub(crate) ci_step_names: Vec<String>,
    pub(crate) ci_gate_count: usize,
    pub(crate) protocol_surface_count: usize,
    pub(crate) agentic: AgenticContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgenticContext {
    pub(crate) instruction_files: Vec<String>,
    pub(crate) config_files: Vec<String>,
    pub(crate) plan_count: usize,
    pub(crate) tasks: Option<AgentTaskCounts>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AgentTaskCounts {
    pub(crate) pending: usize,
    pub(crate) active: usize,
    pub(crate) blocked: usize,
    pub(crate) done: usize,
}
```

`App::new(&Ctx)` populates `ProjectInfo` from the existing `PipelineRunContext`, root path, and
`CiConfig`; `std::env::consts` and `std::thread::available_parallelism` provide portable host
facts. Startup discovery checks common instruction/config paths and summarizes recognized
`status:` lines from `.ctx/godmode/tasks.yaml` or `.ctx/opavs/tasks.yaml`. `App` gains a
crate-private `project_info` field; its existing public fields and methods remain compatible.

## Layout

The Overview tab becomes a project-oriented grid:

1. **Fastfetch header** - a compact taskit ASCII mark beside project name, root, current branch,
   short commit SHA, taskit/Rust/Cargo versions, OS/architecture, and logical CPU count.
2. **Workspace & Pipeline** - crate preview, CI step/gate counts, compact ordered steps, and
   protocol surface count.
3. **Agentic Context** - detected `AGENTS.md`, `CLAUDE.md`, Copilot instructions, `taskit.toml`,
   `Cruxfile`, task graph files, plan count, and pending/active/blocked/done task counts.
4. **Health** - tests, Clippy, TODO/FIXME count, workspace version consistency, and protocol status.
5. **Activity** - latest CI result, run count, duration drift, and interrupted flow state.

At normal widths, the four lower panels render as a two-by-two grid beneath the full-width
fastfetch header. At narrow widths, content is shortened to preserve labels and avoid panics.
Detailed crate rows, CI history, and flow diagnostics remain in their dedicated tabs.

## Data Flow

1. `Ctx` + portable `std` host facts + well-known project files -> `App::new` -> crate-private
   `ProjectInfo` for stable startup metadata and agentic context.
2. Persisted health/telemetry/git state -> `Snapshot::collect` for live status.
3. `App` + `Snapshot` -> `ui::render_overview` -> five populated Ratatui panels.

## Hexagonal Boundaries

No new port or adapter is needed. The design consumes existing engine context and snapshot data;
it introduces no external I/O or dependency.

## Out of Scope

- Running CI, health checks, or other commands from the dashboard.
- New persisted dashboard settings or configuration fields.
- Memory usage, uptime, temperatures, or platform-specific hardware probes.
- Executing or mutating agent task graphs from the dashboard.
- Changes to the Crates, History, or Flow tabs.
- Invented sample telemetry when persisted data is absent; missing activity remains explicit.

## Risk Summary

- [ ] Breaking API changes: no.
- [ ] New external dependency: no.
- [ ] Feature flag required: no.
- [ ] Serialization change: no.
