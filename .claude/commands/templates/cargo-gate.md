# cargo-gate

Run quality gates before committing. Use as a pre-commit check.

## Workflow

1. Run all gates in sequence, stop on first failure:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run --workspace
```

2. If all pass, report success.

3. If any fail, report which gate failed and show the output. Do not
   commit.
