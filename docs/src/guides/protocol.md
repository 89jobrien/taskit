# Protocol Governance

## Contract drift

Configure files whose normalized content forms a contract:

```toml
[[protocol.surfaces]]
name = "pipeline-runner"
path = "crates/taskit-core/src/pipeline_runner.rs"
```

Create or update the lockfile:

```bash
taskit protocol drift --update
```

Check it without mutation:

```bash
taskit protocol drift
taskit protocol drift --warn-only
```

The default lockfile is `taskit-protocol.lock`. Drift fails unless `--warn-only` is set. With no
configured surfaces, normal mode skips successfully.

Watch mode checks repeatedly and updates missing or drifted hashes:

```bash
taskit protocol drift --watch
taskit protocol drift --watch --interval 10
```

The default interval is five seconds. Watch mode mutates the lockfile and runs until interrupted.

Hook mode reads an edited path from hook JSON on stdin, skips paths not configured as protocol
surfaces, and does not fail on drift:

```bash
taskit protocol drift --hook
```

## Construction-site counts

```bash
taskit protocol sites \
  --file crates/taskit-types/src/step.rs \
  --pattern "PipelineOutcome {" \
  --expected 12
```

The command counts lines containing the literal substring and fails when the count differs.
`--warn-only` reports the mismatch without failing.

## TODO/FIXME synchronization

```bash
taskit protocol todo-sync
taskit protocol todo-sync --warn-only
taskit protocol todo-sync --update
```

The scanner covers Rust files under `crates/` and `src/`. Default mode compares against
`taskit-todo-sync.lock`; update mode creates/closes GitHub issues through `gh` and rewrites the
lockfile. Global dry-run prints issue operations and does not write the lockfile.

`--update` is idempotent without the lockfile. Before creating anything it lists the repo's
existing issues and reuses the one already tracking a marker — matching on the issue title plus a
stable `<!-- taskit-todo-sync: <path> -->` tag in the body (issues created before that tag was
added are still matched through their `` `path:line` `` reference). A failed lookup aborts the run
rather than risking a duplicate. Issues created outside `todo-sync` are never adopted.

## Dependency governance

```bash
taskit protocol freshness
taskit protocol freshness --warn-only
taskit protocol audit
```

Freshness runs `cargo outdated --workspace --format json`. If cargo-outdated is unavailable, the
check prints installation guidance and skips successfully. Audit runs `cargo deny check` for
advisories, licenses, and bans.
