# CI Pipeline

Run the full pipeline:

```sh
taskit ci
```

## Steps

| Step | Command | Gate |
|------|---------|------|
| Self-check | `taskit self-check` | Yes |
| Format | `taskit fmt --check` | No |
| Lint | `taskit lint` | No |
| Compile tests | `taskit compile-tests` | No |
| Test | `taskit test` | No |
| Deps | `taskit check-deps` | No |
| Drift | `taskit check-protocol-drift` | No |

## Agentic Flow Pipeline

`taskit flow auto` runs the full branching workflow as a single command:

1. **Promote** — merge `main` into `staging`
2. **CI** — run the full pipeline above against `staging`
3. **Finish** — merge `staging` back into `main` on success

Merge conflicts encountered during promote are passed to `BamlConflictResolver`, which
uses BAML to call an LLM for automatic resolution. If LLM confidence is below the
configured threshold, the command stops and returns `FlowError::NeedsHuman` so a human
can intervene before CI runs.
