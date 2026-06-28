# add-subcommand

Scaffold a new taskit subcommand following project conventions.

## Arguments

- `$ARGUMENTS` -- subcommand name (e.g. `metrics`, `graph`)

## Workflow

1. Parse the subcommand name from `$ARGUMENTS`. If empty, ask the user.

2. Confirm which crate owns the implementation:
   - Most subcommands go in `taskit-engine`
   - Init-related goes in `taskit-init`
   - Shared types go in `taskit-core`

3. Create the module file in the owning crate:
   - `crates/<crate>/src/<name>.rs`
   - Add `pub fn run(...)` entry point
   - Add `#[cfg(test)] mod tests` with at least one test

4. Wire it up:
   - Add `pub mod <name>;` to the crate's `lib.rs`
   - Add `Cmd::<Name>` variant to `src/main.rs`
   - Add dispatch arm in `main.rs` match block

5. Verify:

```bash
cargo check --workspace
cargo nextest run -p <crate> -E 'test(<name>)'
```

6. Show the user the files created and suggest next steps.
