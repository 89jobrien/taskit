#!/usr/bin/env bash
# Smoke-test / interaction driver for the taskit CLI + TUI dashboard.
# Run from the taskit repo root: .claude/skills/run-taskit/driver.sh <cmd>
#
# Commands:
#   build            cargo build --bin taskit
#   help             taskit --help (all subcommand trees)
#   ci-dry           dry-run the full CI pipeline (no side effects)
#   health           run health inspect, both human and JSON output
#   flow             show flow status (branch pipeline position)
#   dashboard        launch the ratatui dashboard in tmux, screenshot it, quit
#   all              run build, help, ci-dry, health, flow, dashboard in order
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
BIN=./target/debug/taskit
TMUX_SESSION=taskit_dash_driver

build() {
  cargo build --bin taskit
}

help() {
  $BIN --help
  for sub in dev check test health protocol release flow self dashboard init; do
    echo "--- taskit $sub --help ---"
    $BIN "$sub" --help || true
  done
}

ci_dry() {
  # --dry-run prints the pipeline plan without executing build/test steps.
  # Note: NOT all subcommands honor --dry-run uniformly (see Gotchas in SKILL.md) —
  # `check ci` is the one verified safe to dry-run.
  $BIN --dry-run check ci
}

health() {
  $BIN health inspect || true
  echo "--- JSON ---"
  $BIN --output json health inspect || true
}

flow() {
  $BIN flow status
}

dashboard() {
  tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
  tmux new-session -d -s "$TMUX_SESSION" -x 200 -y 50 "$BIN dashboard"
  sleep 2
  tmux capture-pane -t "$TMUX_SESSION" -p
  echo "--- switching tab ---"
  tmux send-keys -t "$TMUX_SESSION" Tab
  sleep 1
  tmux capture-pane -t "$TMUX_SESSION" -p | head -5
  echo "--- quitting ---"
  tmux send-keys -t "$TMUX_SESSION" q
  sleep 1
  tmux has-session -t "$TMUX_SESSION" 2>/dev/null && echo "WARN: session still alive" || echo "dashboard quit cleanly"
}

case "${1:-all}" in
  build) build ;;
  help) help ;;
  ci-dry) ci_dry ;;
  health) health ;;
  flow) flow ;;
  dashboard) dashboard ;;
  all) build; flow; ci_dry; health; dashboard ;;
  *) echo "unknown command: $1" >&2; exit 1 ;;
esac
