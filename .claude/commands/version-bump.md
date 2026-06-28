# version-bump

Bump workspace versions consistently across all crates without releasing.

## Arguments

- `$ARGUMENTS` -- required bump level: `patch`, `minor`, `major`, or an
  explicit version like `0.5.0`

## Workflow

1. Parse the target from `$ARGUMENTS`. If empty, ask the user.

2. Read current version from root `Cargo.toml`.

3. Calculate the new version:
   - `patch`: 0.4.0 -> 0.4.1
   - `minor`: 0.4.0 -> 0.5.0
   - `major`: 0.4.0 -> 1.0.0
   - Explicit: use as-is

4. Update version in all 5 `Cargo.toml` files (lockstep):
   - `Cargo.toml` (root)
   - `crates/taskit-core/Cargo.toml`
   - `crates/taskit-engine/Cargo.toml`
   - `crates/taskit-init/Cargo.toml`
   - `crates/taskit-crux/Cargo.toml`

5. Run `cargo check` to verify the workspace builds.

6. Show the diff and ask the user to confirm before committing.

7. If confirmed, commit:

```
chore(release): bump workspace to v<new_version>
```

Do NOT tag or push. Use `/release` for the full flow.
