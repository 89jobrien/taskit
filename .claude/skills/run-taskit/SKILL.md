---
name: run-taskit
description: Build, run, and drive the taskit CLI and its ratatui TUI dashboard. Use when asked to run taskit, build taskit, test taskit's pipeline, check taskit's flow status, or screenshot/interact with the taskit dashboard.
---

# Running taskit

taskit is a config-driven CI-pipeline CLI (`taskit <subcommand>`) with one
interactive surface: `taskit dashboard`, a ratatui TUI. All paths below are
relative to the repo root (`taskit/`).

## Agent path — use the driver

```bash
.claude/skills/run-taskit/driver.sh <cmd>
```

Commands: `build`, `help`, `ci-dry`, `health`, `flow`, `dashboard`, `all`
(runs everything in order). Verified working in this container:

```bash
.claude/skills/run-taskit/driver.sh all
```

- `build` — `cargo build --bin taskit`
- `flow` — `taskit flow status` (branch pipeline position; output goes to **stderr**, not stdout — the CLI logs via `tracing`)
- `ci-dry` — `taskit --dry-run check ci`, prints the full pipeline plan (fmt/lint/compile-tests/test/check-deps/protocol-drift) without executing builds
- `health` — `taskit health inspect` in both human and `--output json` form
- `dashboard` — launches `taskit dashboard` in a detached tmux session, screenshots it via `capture-pane`, switches tabs, sends `q`, and confirms the session exits cleanly

## Prerequisites

- Rust toolchain (edition 2024) already present in this container — `cargo build --bin taskit` succeeded with no extra `apt-get` packages.
- `tmux` — used to drive the TUI dashboard (already present in this container).

## Build

```bash
cargo build --bin taskit
```

Binary lands at `./target/debug/taskit`.

## Run (agent path) — CLI

```bash
./target/debug/taskit --help                 # top-level subcommand tree
./target/debug/taskit flow status             # branch pipeline position (stderr)
./target/debug/taskit --dry-run check ci      # full CI pipeline plan, no execution
./target/debug/taskit health inspect          # pass/fail gate check
./target/debug/taskit --output json health inspect   # same, machine-readable
```

`--output json` is supported workspace-wide and is the reliable way to check
pass/fail programmatically (`.passed` field) instead of parsing stderr text.

## Run (agent path) — TUI dashboard

```bash
tmux new-session -d -s taskit_dash -x 200 -y 50 "./target/debug/taskit dashboard"
sleep 2
tmux capture-pane -t taskit_dash -p     # screenshot as text
tmux send-keys -t taskit_dash Tab       # switch tabs: Overview / Crates / History / Flow
tmux send-keys -t taskit_dash q         # quit
```

Verified in this session: the dashboard renders 4 tabs (Overview, Crates,
History, Flow), `Tab` cycles between them, and `q` exits the tmux pane's
process cleanly (`tmux has-session` returns non-zero afterward).

## Run (human path)

`taskit <subcommand>` directly in a terminal — same binary, no wrapper
needed for non-interactive subcommands. Only `dashboard` needs a real
terminal (or the tmux driver above) since it's a full-screen TUI.

## Test

```bash
cargo nextest run --workspace
```

733 tests passed across the workspace in this session (taskit-core 17,
taskit-crux 4, taskit-engine 501, taskit-init 55, taskit-output 48,
taskit-testing 14, taskit-tui 17, taskit-types 76).

## Gotchas

- **`flow status` and most subcommands print to stderr, not stdout** — they
  route through `tracing`. If you pipe/capture only stdout you'll see
  nothing; capture both streams.
- **`--dry-run` is not honored uniformly.** `taskit --dry-run check ci`
  correctly no-ops the build/test steps and just prints the plan. But
  `taskit --dry-run health check` still ran a real comparison and exited
  non-zero — `--dry-run` only suppresses *pipeline step execution*, not
  every subcommand's internal logic. Don't assume `--dry-run` makes a
  subcommand side-effect-free; verify per-subcommand.
- **This repo currently fails its own `version consistency` health gate**
  (`taskit health inspect` and `taskit check ci` both report `FAIL` on
  "workspace versions are inconsistent"). This is pre-existing repo state,
  not a driver bug — expect non-zero exit codes from `health inspect` and
  `check ci` until that's fixed upstream.
- **`taskit dashboard` needs a real terminal size.** Without `-x`/`-y` on
  `tmux new-session`, ratatui may render into a tiny default pane and the
  screenshot is unreadable. `-x 200 -y 50` gives enough room to see both
  panels on the Overview tab.

## Troubleshooting

No build/launch errors were hit in this container — `cargo build`, all CLI
subcommands, and the tmux-driven dashboard worked on the first attempt.
