# release-check

Validate release readiness for the taskit workspace.

## Workflow

1. Run readiness checks for all crates:

```bash
cargo rail release check taskit --extended
cargo rail release check taskit-core --extended
cargo rail release check taskit-engine --extended
cargo rail release check taskit-init --extended
cargo rail release check taskit-crux --extended
```

2. Also run local quality gates:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
```

3. Summarize results in a table:

| Crate | rail check | fmt | clippy | tests | deny |
| ----- | ---------- | --- | ------ | ----- | ---- |

4. If any check fails, report the failure clearly and stop. Do not proceed
   to release.
