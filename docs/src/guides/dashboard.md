# Dashboard

Launch the live terminal dashboard with:

```bash
taskit dashboard
```

The event loop polls every 500 milliseconds. Workspace metadata is collected once at startup;
health, telemetry, flow, and protocol state refresh on each tick without running tests or Clippy.

## Tabs

| Tab      | Contents                                                                         |
| -------- | -------------------------------------------------------------------------------- |
| Overview | Project/system facts, workspace pipeline, agentic context, health, and activity. |
| Crates   | Cargo workspace members.                                                         |
| History  | Seven days of telemetry records.                                                 |
| Flow     | Branch hops, resumable state, resolver, and `flow auto` telemetry.               |

The Overview recognizes common agent files including `AGENTS.md`, `CLAUDE.md`, `taskit.toml`,
`Cruxfile`, and godmode/OPAVS task and plan paths.

## Controls

| Key                    | Behavior                                                  |
| ---------------------- | --------------------------------------------------------- |
| `Tab`, `Right`, `l`    | Next tab                                                  |
| `BackTab`, `Left`, `h` | Previous tab                                              |
| `j`/`Down`, `k`/`Up`   | Scroll or select an action                                |
| `PageDown`, `PageUp`   | Scroll ten rows                                           |
| `g`/`Home`, `G`/`End`  | Jump to start or end                                      |
| `a`                    | Open or close Actions                                     |
| `Enter`                | Start the selected action                                 |
| `x`                    | Cancel after reopening Actions while an action is running |
| `Esc`                  | Close Actions; otherwise quit                             |
| `q`, `Ctrl-C`          | Quit                                                      |

## Health actions

Actions runs one of these fixed commands through `rx-runner`:

```bash
taskit health check --update
taskit health check --update --with-coverage
```

The dashboard remains responsive and shows elapsed time, completion status, and recent output.
Both actions replace `.health-baseline.json`; coverage also performs an instrumented workspace
build. Dashboard `--dry-run` does not propagate to these child commands.

## Data files

```text
.health-baseline.json
.taskit/telemetry/YYYY/MM/DD/history.ndjson
target/taskit/state.json
taskit-protocol.lock
```

Missing baseline, telemetry, or flow state appears as unavailable or empty instead of terminating
the event loop. Protocol lock errors currently produce a configured status with no drifted
surfaces, which the Overview renders as in sync. Trend arrays retain up to 30 recent values.

For the crate API and internal data model, see [taskit-tui](../crates/taskit-tui.md).
