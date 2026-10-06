# Testing and Coverage

## Run tests

```bash
taskit test run
taskit test run --crate-name taskit-engine
taskit test run --affected
taskit test run --offline
taskit test run --continue-on-error
```

The default runs `cargo nextest` for the workspace with the lockfile and fail-fast behavior.
`--crate-name` takes precedence over `--affected`; `--continue-on-error` selects nextest's
`--no-fail-fast`. Offline mode applies `[workspace].offline_skip` when configured.

## Coverage

```bash
taskit test coverage --crate-name taskit-engine
taskit test coverage --workspace
taskit test coverage --threshold 85
```

Coverage uses `cargo llvm-cov` and fails below the line-coverage threshold, which defaults to 80.
Without `--workspace`, the package comes from `--crate-name` or `[coverage].crate_name`; omitting
both is an error. Dry-run prints the command and skips measurement.

Generate the workspace HTML report with:

```bash
taskit test report
```

The report is written to:

```text
target/llvm-cov/html/index.html
```

The current report command prints a non-zero coverage subprocess status but still returns success
after reporting the path. Failure to start the subprocess remains an error. Global dry-run does
not suppress this command; it still executes `cargo llvm-cov` and can write report artifacts.

## Property tests

```bash
taskit test proptest --crate-name taskit-types
```

This runs nextest with the expression `test(prop)` for the selected package.

## Fuzzing

```bash
taskit test fuzz fuzz_config
taskit test fuzz fuzz_config --duration 120
```

The default duration is 60 seconds. The command invokes `cargo fuzz run` with
`-max_total_time=<SECONDS>` and may write corpus or artifact data.

## Benchmarks

```bash
taskit test bench
taskit test bench --crate-name taskit-engine
taskit test bench --save-baseline
```

The baseline option forwards `--save-baseline main` to Criterion and writes benchmark baseline
data.

## Snapshot review

```bash
taskit test snapshots
```

This launches `cargo insta review`. It is interactive and can accept or reject pending snapshots.
