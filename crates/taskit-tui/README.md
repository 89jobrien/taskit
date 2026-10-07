# taskit-tui

`taskit-tui` provides the interactive governance dashboard launched by `taskit dashboard`. It
combines workspace identity, health baselines, CI telemetry, branch-flow state, protocol drift, and
explicit health-update actions in a Ratatui/Crossterm interface.

## Workspace role

The crate is a presentation adapter over `taskit-engine` and `taskit-types`. It reads persisted
state for normal refreshes and uses `rx-runner` only when the user explicitly starts an action. The
root taskit binary owns CLI dispatch and passes an initialized engine `Ctx` to `taskit_tui::run`.

The crate has no Cargo feature flags.

## Running the dashboard

```bash
taskit dashboard
```

The command accepts taskit's global `--dry-run` and `--output` options because they are global Clap
arguments, but neither changes the interactive dashboard behavior. The event loop polls every 500
milliseconds; collection and rendering time can extend a refresh interval.

`run` enables raw mode, enters the alternate screen, installs a panic hook that attempts terminal
restoration, and starts the event loop. Once terminal construction succeeds, both successful and
error exits make a best-effort attempt to leave raw/alternate-screen mode and restore the cursor.

## Views

| Tab      | Contents                                                                    |
| -------- | --------------------------------------------------------------------------- |
| Overview | Project, workspace, agentic context, health, and recent activity summaries. |
| Crates   | Cargo workspace member package names collected at startup.                  |
| History  | Full telemetry records from the last seven days.                            |
| Flow     | Branch hops, resolver mode, resumable state, and recent flow telemetry.     |

Startup metadata includes the project root/name, branch commit, OS/architecture, logical CPU
count, taskit/Rust/Cargo versions, configured CI steps/gates, protocol surface count, and detected
agent instruction/config/task/plan files. Cargo metadata is collected once at startup rather than
on every refresh.

## Keyboard controls

| Key                     | Action                                           |
| ----------------------- | ------------------------------------------------ |
| `Tab`, `Right`, `l`     | Select the next tab.                             |
| `BackTab`, `Left`, `h`  | Select the previous tab.                         |
| `j`, `Down`, `k`, `Up`  | Scroll content or move the action selection.     |
| `PageDown`, `PageUp`    | Move ten rows in a scrollable tab.               |
| `g`, `Home`, `G`, `End` | Jump to the start or end of scrollable content.  |
| `a`                     | Open or close the Actions modal.                 |
| `Enter`                 | Start the selected action.                       |
| `x`                     | Cancel the running action while Actions is open. |
| `Esc`                   | Close Actions; otherwise quit.                   |
| `q`, `Ctrl-C`           | Quit.                                            |

## Snapshot data flow

`Snapshot::collect` performs best-effort reads on each tick:

- `.health-baseline.json` through `taskit_engine::health`;
- taskit telemetry records from the previous seven days;
- git-flow status and resumable `target/taskit/state.json` state;
- configured protocol-surface drift; and
- a local `HH:MM:SS` timestamp from the platform `date` command.

Missing or invalid persisted state becomes `None` or an empty collection rather than terminating
the UI. Snapshot collection does not run Clippy, tests, coverage, or the CI pipeline. It does query
git flow/protocol state and invokes `date`; the crate-level documentation's phrase "all data is read
from disk" should therefore be interpreted as "no expensive quality pipeline is rerun."

CI and flow sparkline histories retain the latest 30 metric values. Drift compares the latest CI
duration with earlier samples in the seven-day window.

## Actions

The Actions modal provides two fixed commands:

```bash
taskit health check --update
taskit health check --update --with-coverage
```

The controller launches the current taskit executable in the workspace root through `rx-runner`,
polls it without blocking refreshes, and tracks idle/running/succeeded/failed/cancelled states. It
retains the latest six output lines. Only one action can run at a time. Action types and controller
state are crate-private; they are UI implementation details rather than public embedding APIs.

## Public API

The crate root exports `run`, `App`, `Tab`, and `Snapshot`.

### `run`

```rust
use taskit_engine::Ctx;
use taskit_types::error::TaskitError;

fn open_dashboard(ctx: &Ctx) -> Result<(), TaskitError> {
    taskit_tui::run(ctx)
}
```

### `Tab`

`Tab` has `Overview`, `Crates`, `History`, and `Flow` variants. `Tab::ALL` defines cycling order,
and `title` returns the displayed name.

```rust
use taskit_tui::Tab;

assert_eq!(Tab::ALL.len(), 4);
assert_eq!(Tab::History.title(), "History");
```

### `App`

`App` exposes `active_tab`, `crate_names`, and `scroll`. `App::new` collects startup metadata.
Public methods cycle tabs, move by rows/pages, jump to top/bottom, and clamp scroll against a
`Snapshot`. Project metadata and action state remain crate-private.

### `Snapshot`

`Snapshot::collect(&Ctx)` returns the current best-effort model. Public fields expose the baseline,
CI sample counts/results/drift/history, telemetry records, flow status/state/resolver/history, last
resolved-conflict count, protocol drift, and refresh time. Consumers should tolerate absent data.

## Source layout

| Module        | Responsibility                                                   |
| ------------- | ---------------------------------------------------------------- |
| `app.rs`      | Tabs, scrolling, startup metadata, and detected agentic context. |
| `snapshot.rs` | Read-only health, telemetry, flow, and protocol model.           |
| `action.rs`   | Non-blocking health action lifecycle and retained output.        |
| `ui.rs`       | Ratatui layout, panels, sparklines, footer, and modal.           |
| `lib.rs`      | Terminal lifecycle, refresh loop, and key routing.               |

## Development and testing

```bash
cargo check -p taskit-tui
cargo clippy -p taskit-tui --all-targets -- -D warnings
cargo nextest run -p taskit-tui
```

Tests cover tab/scroll behavior, agentic file detection, snapshot metric aggregation, action command
construction and lifecycle, key routing, and rendering through Ratatui's test backend. Manual
testing should also verify terminal restoration after normal quit, an action failure, and Ctrl-C.
