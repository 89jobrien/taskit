# explore-crate

Read and summarize a workspace crate's structure and public API.

## Arguments

- `$ARGUMENTS` -- crate name (e.g. `taskit-engine`, `taskit-core`)

## Workflow

1. Parse the crate name from `$ARGUMENTS`. If empty, list available crates
   and ask the user to pick one.

2. Read the crate's `Cargo.toml` for dependencies and features.

3. Read `src/lib.rs` (or `src/main.rs`) for the module tree and public API.

4. Count tests:

```bash
cargo nextest list -p <crate> 2>/dev/null | tail -1
```

5. Summarize:
   - Purpose (from module docs or CLAUDE.md crate table)
   - Public types and functions
   - Dependencies (direct only)
   - Test count
   - Key modules and their roles
