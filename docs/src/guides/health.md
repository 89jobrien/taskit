# Health and Baselines

## Collect health

```bash
taskit health check
```

Health collection measures:

- nextest totals
- Clippy warnings and errors
- TODO/FIXME counts under `crates/` and `src/`
- `.unwrap()`/`.expect()` and `warn!()` call sites
- workspace crate count and version consistency
- latest CI duration telemetry
- optional workspace coverage

Without an existing baseline, the command prints current health and succeeds.

## Update the baseline

```bash
taskit health check --update
taskit health check --update --with-coverage
```

Both commands replace:

```text
.health-baseline.json
```

Coverage mode also runs an instrumented workspace build. Normal comparison reports regressions in
tests, Clippy, TODO/FIXME, safety calls, version consistency, and CI duration. Coverage is compared
only when `--with-coverage` supplies a current reading. Crate-count changes are informational.

## Safety gate

```bash
taskit health check --gate
```

Gate mode compares only increases in `.unwrap()`/`.expect()` and `warn!()` counts. It still runs
the full nextest and Clippy collection; it narrows comparison, not collection cost.

> `taskit --dry-run health check` does not suppress nextest, Clippy, source scans, or metadata.
> With `--update`, it still overwrites `.health-baseline.json`. Coverage collection alone honors
> dry-run.

## Historical metric drift

```bash
taskit health drift --metric ci_duration_ms
taskit health drift --metric ci_duration_ms --window 14
```

The default window is seven days. At least two matching readings are required. The newest value is
compared with the nearest-rank 95th percentile of earlier readings and fails above 120% of that
baseline. Telemetry is read from:

```text
.taskit/telemetry/YYYY/MM/DD/history.ndjson
```

## Threshold inspection

```bash
taskit health inspect
taskit health inspect --max-warnings 0 --max-todo 10
```

Inspect checks test failures, Clippy errors/warnings, version consistency, and optionally
TODO/FIXME. CLI warning/TODO limits override `[inspect]`; absent TODO limits disable that check.

## Version inventory

```bash
taskit health version
```

This prints Cargo workspace package versions and `rustc --version`.
