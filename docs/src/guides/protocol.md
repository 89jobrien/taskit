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

A marker may also name its issue directly with a trailing `(#47)` citation. A citation that
resolves to an issue in the repo wins over title matching, because the author named the issue they
meant. Citations are often plan-document numbering rather than GitHub numbers, so an unresolved
citation falls back to title-plus-file matching rather than dead-ending.

### What counts as a marker

Only a `TODO` or `FIXME` that _begins_ a comment is a marker. A citation inside prose — for
example `// see TODO(unify) in chain_runner.rs:40` — reports on a marker that lives elsewhere, so
tracking it would file a second issue for that marker. Leading `/` and `*` are stripped first, so
doc comments (`///`) and block-comment bodies (`*`) still count.

### Closing duplicates

```bash
taskit protocol todo-dedupe
taskit protocol todo-dedupe --update
```

When the same marker has accumulated several auto-generated issues, `todo-dedupe` keeps the
lowest-numbered one, closes the rest with a comment pointing at the survivor, and repoints the
lockfile entry. Grouping is derived from the same `(title, file)` signal `todo-sync` uses, so an
issue a human opened is never closed. Read-only unless `--update` is set.

A closed issue is reported but never reopened automatically. The closure may predate the work — a
bulk sweep can close generated issues without the fix ever landing — and only a human can tell, so
`todo-dedupe` reports the file and number and leaves the decision open.

## Dependency governance

```bash
taskit protocol freshness
taskit protocol freshness --warn-only
taskit protocol audit
```

Freshness runs `cargo outdated --workspace --format json`. If cargo-outdated is unavailable, the
check prints installation guidance and skips successfully. Audit runs `cargo deny check` for
advisories, licenses, and bans.
