# Design: Remove xtask shim, rename cache directory

## Goal

Remove the generated xtask crate pattern entirely. Users call
`taskit` directly (or `cargo taskit` via alias). Rename
`.xtask-cache/` to `.taskit-cache/` for consistency.

## Approved Approach

**B -- Clean removal + cargo alias.** Delete the xtask shim crate and
its generation code. Keep `.cargo/config.toml` generation with alias
`taskit = "run --package taskit --"` so `cargo taskit ci` works.

## Scope

### Deletions

| Target                                            | Reason                      |
| ------------------------------------------------- | --------------------------- |
| `xtask/` directory                                | Shim crate no longer needed |
| `write_xtask_crate()` in `taskit-init/src/lib.rs` | Generator removed           |
| `"xtask"` in workspace members (`Cargo.toml`)     | No longer a member          |

### Modifications -- taskit-init

| File                          | Change                                                                                                   |
| ----------------------------- | -------------------------------------------------------------------------------------------------------- |
| `taskit-init/src/lib.rs`      | Remove `write_xtask_crate()` call from `run_init()`, update "Next steps" output                          |
| `taskit-init/src/lib.rs`      | `write_cargo_alias()`: change `xtask = "run --package xtask --"` to `taskit = "run --package taskit --"` |
| `taskit-init/src/scaffold.rs` | `write_git_hooks()`: `exec cargo xtask pre-commit` -> `exec taskit pre-commit`                           |
| `taskit-init/src/scaffold.rs` | `write_github_ci()`: `cargo xtask ci` -> `taskit ci`                                                     |

### Modifications -- taskit-engine

| File                   | Change                                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------ |
| `lint.rs`              | Remove `--exclude xtask` from clippy commands                                                          |
| `testing/self_test.rs` | Rename `XTASK_SRC` -> test taskit's own source; rename cache paths `.xtask-cache/` -> `.taskit-cache/` |
| `testing/compile.rs`   | `.xtask-cache` -> `.taskit-cache` in cache dir, exclusion list                                         |
| `hooks.rs`             | `cargo xtask pre-commit/pre-push` -> `taskit pre-commit/pre-push` in generated hooks and cache paths   |
| `dev_setup.rs`         | `.xtask-cache` -> `.taskit-cache`; `cargo xtask dev-setup` -> `taskit dev-setup`                       |
| `protocol/drift.rs`    | `cargo xtask check-protocol-drift` -> `taskit check-protocol-drift`                                    |
| `runner.rs`            | `cargo xtask quick` -> `taskit quick` in doc comment                                                   |
| `quick.rs`             | `cargo xtask ci` -> `taskit ci` in doc comment                                                         |
| `clean.rs`             | `.xtask-cache` -> `.taskit-cache`                                                                      |
| `cache/mod.rs`         | `.xtask-cache` -> `.taskit-cache`; `xtask/master-hash` -> `taskit/master-hash` or inline               |
| `affected.rs`          | Update test assertion for xtask path                                                                   |
| `util.rs`              | Update comment and test names                                                                          |

### Modifications -- root project

| File                   | Change                                  |
| ---------------------- | --------------------------------------- |
| `Cargo.toml`           | Remove `"xtask"` from workspace members |
| `src/main.rs`          | `about` string: remove "xtask" wording  |
| `.cargo/config.toml`   | `taskit = "run --package taskit --"`    |
| `.githooks/pre-commit` | `exec taskit pre-commit`                |
| `.githooks/pre-push`   | `exec taskit pre-push`                  |

### Modifications -- docs

| File                                | Change                                    |
| ----------------------------------- | ----------------------------------------- |
| `README.md`                         | Remove xtask references                   |
| `CLAUDE.md`                         | Remove xtask references                   |
| `AGENTS.md`                         | Remove `cargo xtask pre-commit` reference |
| `DESIGN.md`                         | Rewrite to remove xtask framing           |
| `CHANGELOG.md`                      | Regenerate from git history               |
| `docs/src/reference/ci-pipeline.md` | `cargo xtask ci` -> `taskit ci`           |

### Modifications -- crate changelogs

| File                                | Change     |
| ----------------------------------- | ---------- |
| `crates/taskit-init/CHANGELOG.md`   | Regenerate |
| `crates/taskit-engine/CHANGELOG.md` | Regenerate |

## Cache Directory Rename

`.xtask-cache/` -> `.taskit-cache/` everywhere:

- Constant definitions in `cache/mod.rs`, `hooks.rs`, `compile.rs`,
  `self_test.rs`, `clean.rs`, `dev_setup.rs`
- `.gitignore` entries that reference `.xtask-cache`
- The `MASTER_FILE` constant changes from `xtask/master-hash` to
  a path that doesn't reference a deleted directory (e.g.
  `.taskit-cache/master-hash`)

## Public API

No new types, traits, or functions. This is purely removal and rename.

## Hexagonal Boundaries

No changes. No new ports or adapters.

## Out of Scope

- Adding new taskit subcommands
- Changing pipeline behavior
- `docs/designs/` and `docs/plans/` historical files (left as-is)

## Risk

- [ ] Breaking API changes: no (binary CLI unchanged, just wording)
- [ ] New external dependency: no
- [ ] Feature flag required: no
- [ ] Cache migration: users with existing `.xtask-cache/` will need
      to delete it or run `taskit clean`. No auto-migration needed
      since the cache is ephemeral.
- [ ] Semver: this is a minor-bump change (removes generated output,
      renames internal cache dir). No library API breakage.
