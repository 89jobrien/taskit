# CI Pipeline

```bash
taskit check ci
taskit check ci --fail-fast
taskit check ci --include-network
```

## Built-in pipeline

When `[ci]` is absent, taskit runs:

| Order | Step           | Equivalent public command                   | Gate |
| ----- | -------------- | ------------------------------------------- | ---- |
| 1     | Self-check     | `taskit self check`                         | yes  |
| 2     | Format         | `taskit check fmt --check`                  | no   |
| 3     | Lint           | `taskit check lint`                         | no   |
| 4     | Compile tests  | `taskit check compile`                      | no   |
| 5     | Test           | `taskit test run`                           | no   |
| 6     | Dependencies   | `taskit check deps`                         | no   |
| 7     | Protocol drift | `taskit protocol drift`                     | no   |
| 8     | Coverage       | `taskit test coverage --crate-name <CRATE>` | no   |

Coverage is added only when `[coverage]` exists.

## Configured pipeline

An explicit `[ci]` section replaces the built-in sequence. Supported `cmd` values are:

```text
fmt
fmt --check
lint
compile-tests
test
coverage
check-deps
check-protocol-drift
self-check
health
health-gate
```

Unknown command names fail pipeline construction. `CiConfig.cruxfile` is currently parsed but does
not select `BuiltinRunner`, `SubprocessCruxRunner`, or `EmbeddedCruxRunner`.

## Gates and fail-fast

- A failed gate skips every remaining step.
- With fail-fast, any failed step skips the remainder.
- Without fail-fast, ordinary failures are recorded and later ordinary steps continue.
- Effective fail-fast is CLI `--fail-fast` or `[ci].fail_fast = true`.

## Offline filtering

CI excludes configured network/credential tests by default. Filtering is active only when
`[workspace].offline_skip` contains a nextest expression. `--include-network` disables it.

## Telemetry

Each non-dry CI run attempts to record:

```text
ci_duration_ms
ci_passed
```

Records are appended under `.taskit/telemetry/YYYY/MM/DD/history.ndjson`.
