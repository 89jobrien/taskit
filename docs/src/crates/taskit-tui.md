# taskit-tui

`taskit-tui` provides the interactive terminal dashboard launched by `taskit dashboard`. It
renders project metadata, workspace configuration, health baselines, CI telemetry, protocol
drift, flow state, and non-blocking health-update actions.

The crate uses Rust 2024 and depends on `taskit-engine`, `taskit-types`, Ratatui, Crossterm, and
`rx-runner`.

## Contents

- [Run the dashboard](#run-the-dashboard)
- [Tabs](#tabs)
- [Keyboard controls](#keyboard-controls)
- [Health actions](#health-actions)
- [Architecture and data flow](#architecture-and-data-flow)
- [Public API](#public-api)

## Run the dashboard

```bash
taskit dashboard
```

The dashboard command accepts taskit's global options:

| Option              | Dashboard behavior                                                       |
| ------------------- | ------------------------------------------------------------------------ |
| `--dry-run`         | Accepted by the global parser; it does not change dashboard behavior.    |
| `--output <FORMAT>` | Accepted by the global parser; the dashboard remains an interactive TUI. |

The event loop polls every 500 milliseconds; collection and rendering time can extend an
individual refresh. `q` and `Ctrl-C` always quit. `Esc` quits when Actions is closed.

## Tabs

| Tab      | Contents                                                                           |
| -------- | ---------------------------------------------------------------------------------- |
| Overview | Project, workspace, agentic, health, and activity summaries.                       |
| Crates   | Workspace member names returned by `cargo metadata`.                               |
| History  | Telemetry records from the last seven days.                                        |
| Flow     | Branch hops, conflict resolver, resumable state, and recent `flow auto` telemetry. |

The Overview header shows the project root, branch and short commit SHA, operating system,
architecture, logical CPU count, and taskit/Rust/Cargo versions. Agentic Context reports detected
instruction/config files, plans, and recognized task-status counts.

## Keyboard controls

| Key                     | Action                                                     |
| ----------------------- | ---------------------------------------------------------- |
| `Tab`, `Right`, `l`     | Select the next tab.                                       |
| `BackTab`, `Left`, `h`  | Select the previous tab.                                   |
| `j`, `Down`, `k`, `Up`  | Scroll Crates/History or move the Actions selection.       |
| `PageDown`, `PageUp`    | Move ten rows in a scrollable tab.                         |
| `g`, `Home`, `G`, `End` | Jump to the start or end of a scrollable tab.              |
| `a`                     | Open or close the Actions modal.                           |
| `Enter`                 | Start the selected action.                                 |
| `x`                     | Cancel the running action while the Actions modal is open. |
| `Esc`                   | Close Actions; otherwise quit the dashboard.               |
| `q`, `Ctrl-C`           | Quit the dashboard.                                        |

## Health actions

The Actions modal exposes two fixed commands:

```bash
taskit health check --update
taskit health check --update --with-coverage
```

Commands run through `rx-runner` without blocking dashboard refreshes. The Activity panel shows
the active action, elapsed time, terminal result, and latest retained output line. The controller
retains at most six output lines for the current action. A completed update is visible when the
next snapshot reloads `.health-baseline.json`, whose path is owned by
`crates/taskit-engine/src/health.rs`.

## Architecture and data flow

| Module                              | Responsibility                                             |
| ----------------------------------- | ---------------------------------------------------------- |
| `crates/taskit-tui/src/app.rs`      | Tab/scroll state and startup project/agentic metadata.     |
| `crates/taskit-tui/src/snapshot.rs` | Read-only health, telemetry, flow, and protocol snapshot.  |
| `crates/taskit-tui/src/action.rs`   | Health actions, process lifecycle, and status.             |
| `crates/taskit-tui/src/ui.rs`       | Ratatui widget layout for tabs, panels, footer, and modal. |
| `crates/taskit-tui/src/lib.rs`      | Terminal setup/teardown, refresh loop, and key routing.    |

`App::new` gathers startup metadata once through `Ctx::pipeline_run_context`, including workspace
members from `cargo metadata`, tool versions, the workspace root, and the current commit. It also
checks well-known project files for agent instructions, task graphs, plans, and taskit/Crux
configuration.

`Snapshot::collect` refreshes the following read-only state on every tick:

- `.health-baseline.json`
- telemetry records from the last seven days
- flow branch positions and resumable `target/taskit/state.json`, loaded by
  `crates/taskit-engine/src/flow_state_store.rs`
- configured protocol-surface drift
- a local `HH:MM:SS` timestamp from `date +%H:%M:%S`

Snapshot collection does not run Clippy, tests, coverage, or a CI pipeline. Those expensive
operations run only when explicitly selected from Actions or invoked through another taskit
command.

## Public API

The crate root exports `run`, `App`, `Tab`, and `Snapshot`. Dashboard actions and rendering helpers
remain crate-private.

### `run`

```text
taskit_tui::run(ctx: &taskit_engine::ctx::Ctx)
    -> Result<(), taskit_types::error::TaskitError>
```

Enters raw mode and the alternate screen, then runs the event loop. Once terminal construction
succeeds, normal and error returns from the event loop perform best-effort terminal restoration.
Terminal setup, event reads, and drawing failures are returned as `TaskitError`; a panic hook also
attempts restoration before the default panic output.

```rust
use taskit_engine::ctx::Ctx;
use taskit_types::error::TaskitError;

fn open_dashboard(ctx: &Ctx) -> Result<(), TaskitError> {
    taskit_tui::run(ctx)
}
```

### `Tab`

```rust
pub enum Tab {
    Overview,
    Crates,
    History,
    Flow,
}
```

| Export                             | Purpose                                           |
| ---------------------------------- | ------------------------------------------------- |
| `Tab::ALL`                         | Ordered four-tab array used for keyboard cycling. |
| `Tab::title(self) -> &'static str` | Return the displayed tab title.                   |

```rust
use taskit_tui::Tab;

assert_eq!(Tab::ALL.len(), 4);
assert_eq!(Tab::History.title(), "History");
```

### `App`

`App` owns user-controlled session state. Its public fields are `active_tab: Tab`,
`crate_names: Vec<String>`, and `scroll: u16`.

| Method                                                      | Purpose                                                      |
| ----------------------------------------------------------- | ------------------------------------------------------------ |
| `App::new(ctx: &Ctx) -> App`                                | Collect startup project metadata and initialize Overview.    |
| `next_tab(&mut self)`                                       | Select the next tab, wrapping to Overview, and reset scroll. |
| `prev_tab(&mut self)`                                       | Select the previous tab, wrapping to Flow, and reset scroll. |
| `scroll_down(&mut self)` / `scroll_up(&mut self)`           | Move one row with saturating arithmetic.                     |
| `scroll_down_page(&mut self)` / `scroll_up_page(&mut self)` | Move ten rows.                                               |
| `scroll_top(&mut self)` / `scroll_bottom(&mut self)`        | Request the first or final row.                              |
| `clamp_scroll(&mut self, snapshot: &Snapshot)`              | Clamp scroll to active-tab content.                          |

```rust
use taskit_engine::ctx::Ctx;
use taskit_tui::{App, Snapshot, Tab};

fn select_history(ctx: &Ctx) {
    let mut app = App::new(ctx);
    let snapshot = Snapshot::collect(ctx);
    app.active_tab = Tab::History;
    app.scroll_bottom();
    app.clamp_scroll(&snapshot);
}
```

### `Snapshot`

```text
Snapshot::collect(ctx: &taskit_engine::ctx::Ctx) -> Snapshot
```

`collect` performs best-effort reads. Missing or invalid persisted data becomes `None` or an empty
collection rather than an error returned to the event loop.

| Public field                                   | Meaning                                         |
| ---------------------------------------------- | ----------------------------------------------- |
| `refreshed_at: String`                         | Local refresh time formatted as `HH:MM:SS`.     |
| `baseline: Option<HealthBaseline>`             | Last stored health baseline.                    |
| `ci_run_count: usize`                          | CI pass/fail samples in the seven-day window.   |
| `last_ci_passed: Option<bool>`                 | Most recent CI result.                          |
| `ci_duration_drift: Option<DriftReport>`       | Latest duration compared with prior samples.    |
| `ci_duration_history: Vec<u64>`                | Up to 30 recent CI duration readings.           |
| `ci_passed_history: Vec<u64>`                  | Up to 30 recent CI pass/fail readings.          |
| `records: Vec<TelemetryRecord>`                | Full telemetry records in the seven-day window. |
| `flow_status: Option<FlowStatusReport>`        | Current branch and configured flow hops.        |
| `flow_state: Option<FlowState>`                | Interrupted `flow auto` state, if present.      |
| `flow_conflict_resolver: ConflictResolverKind` | Configured conflict resolver.                   |
| `flow_auto_duration_history: Vec<u64>`         | Up to 30 recent `flow auto` durations.          |
| `flow_auto_result_history: Vec<u64>`           | Up to 30 recent `flow auto` results.            |
| `flow_auto_conflicts_last: Option<u64>`        | Last recorded resolved-conflict count.          |
| `protocol_drift: Option<ProtocolDriftStatus>`  | Current drift state for protocol surfaces.      |

```rust
use taskit_engine::ctx::Ctx;
use taskit_tui::Snapshot;

fn latest_ci_result(ctx: &Ctx) -> Option<bool> {
    Snapshot::collect(ctx).last_ci_passed
}
```
