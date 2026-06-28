# release-init

Initialize cargo-rail release configuration for the taskit workspace.

## Workflow

1. Run `cargo rail release init` for each crate in the workspace:

```bash
cargo rail release init taskit
cargo rail release init taskit-core
cargo rail release init taskit-engine
cargo rail release init taskit-init
cargo rail release init taskit-crux
```

2. Verify a `rail.toml` was created or updated at the workspace root.

3. Read the generated `rail.toml` and confirm it looks correct.

4. If `rail.toml` already exists, inform the user and ask before overwriting.
