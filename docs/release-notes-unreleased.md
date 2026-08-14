# Release Notes — Unreleased

Changes on `develop` since the last dev-facing changelog update, not yet tagged.

## What's New

- **Sync TODO/FIXME markers to GitHub issues.** The new `taskit todo-sync`
  command scans your source for TODO/FIXME comments and keeps a matching set
  of GitHub issues in sync — creating an issue for each new marker and
  closing the issue when the marker is removed from source. Run it read-only
  to check for drift, or with `--update` to actually create/close issues.
- **A new Flow tab in the TUI dashboard.** `taskit dashboard` now has a Flow
  tab showing your current position in the git-flow pipeline, any in-progress
  `flow auto` run you can resume, which conflict resolver is configured, and
  recent `flow auto` run history. The Overview tab also now shows a
  protocol-drift status line alongside existing health metrics.

## Improvements

- **`taskit lint --fix`** auto-applies clippy's suggested fixes instead of
  just reporting them.
- **`taskit coverage --workspace`** measures line coverage across the whole
  workspace in one run, instead of one crate at a time.
- **`taskit check-protocol-drift --watch`** continuously watches for drift
  and auto-fixes it, instead of a one-shot pass/fail check.
- **`taskit check-freshness --warn-only`** reports outdated dependencies
  without failing the command — useful for visibility without blocking CI.
- **`taskit health --with-coverage`** adds an optional workspace coverage
  measurement to the health report, alongside new tracking for
  `.unwrap()`/`.expect()` call-site counts and recent CI run duration.
- Shell completions can now be generated for Nushell, in addition to the
  shells already supported.

## Contributors

Joseph O'Brien
