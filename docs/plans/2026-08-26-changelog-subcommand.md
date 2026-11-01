# Plan: Changelog Subcommand

## Goal

Add a top-level, typed `taskit changelog` wrapper around the repository's `git-cliff` workflow.

## Context Map

The complete context map is recorded in
`docs/designs/2026-08-26-changelog-subcommand-design.md`.

## Architecture

- Crates affected: `taskit-engine` and the root `taskit` binary.
- New type: `taskit_engine::changelog::ChangelogMode`.
- New command: `taskit_engine::command::Changelog`.
- Data flow: Clap mode -> engine mode -> fixed `git-cliff` arguments -> `Ctx::run`.
- Runtime prerequisite: installed `git-cliff`; no new Cargo dependencies.

## Task

### Task 1: Implement typed changelog workflows

**Crate**: `taskit-engine`, `taskit`

**Files**:

- `src/main.rs`
- `crates/taskit-engine/src/changelog.rs`
- `crates/taskit-engine/src/command.rs`
- `crates/taskit-engine/src/lib.rs`

**Run**: `cargo nextest run -p taskit-engine && cargo test -p taskit --bin taskit`

1. Add a failing parser acceptance test to `src/main.rs`:

   ```rust
   #[test]
   fn changelog_modes_parse() {
       for args in [
           vec!["taskit", "changelog"],
           vec!["taskit", "changelog", "unreleased"],
           vec!["taskit", "changelog", "full"],
           vec!["taskit", "changelog", "latest"],
           vec!["taskit", "changelog", "preview"],
       ] {
           assert!(Cli::try_parse_from(args).is_ok());
       }
   }
   ```

2. Run `cargo test -p taskit --bin taskit changelog_modes_parse -- --exact` and verify the
   assertion fails because `changelog` is not recognized.

3. Add `crates/taskit-engine/src/changelog.rs` with:

   ```rust
   #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
   pub enum ChangelogMode {
       #[default]
       Unreleased,
       Full,
       Latest,
       Preview,
   }

   pub fn run(ctx: &Ctx, mode: ChangelogMode) -> Result<(), TaskitError>;
   ```

   The function checks `ctx.root().join("cliff.toml")`, returns a `TaskitError` when absent,
   and delegates these exact commands through `Ctx::run`:

   | Mode         | Command                                         |
   | ------------ | ----------------------------------------------- |
   | `Unreleased` | `git-cliff --unreleased --prepend CHANGELOG.md` |
   | `Full`       | `git-cliff -o CHANGELOG.md`                     |
   | `Latest`     | `git-cliff --latest --prepend CHANGELOG.md`     |
   | `Preview`    | `git-cliff --unreleased`                        |

   Add dry-run unit tests that inspect `Ctx::command_capture_finish` for each exact command,
   plus a missing-configuration error test.

4. Export `pub mod changelog;` from `crates/taskit-engine/src/lib.rs`.

5. Add this command adapter to `crates/taskit-engine/src/command.rs`:

   ```rust
   pub struct Changelog {
       pub mode: changelog::ChangelogMode,
   }

   impl Command for Changelog {
       fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
           changelog::run(ctx, self.mode)
       }
   }
   ```

6. Add a private `ChangelogCmd` Clap subcommand enum in `src/main.rs`, add
   `Cmd::Changelog { sub: Option<ChangelogCmd> }`, and map omitted mode to
   `ChangelogMode::Unreleased` in `to_command`.

7. Verify:

   ```text
   cargo fmt --all --check
   cargo check --workspace
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo nextest run --workspace
   cargo run -p taskit -- --dry-run changelog preview
   ```

8. Do not commit; committing requires a separate explicit user request.

## Risk

- Existing user changes are present in three files this task modifies; preserve them exactly.
- `cliff.toml` validation must use `Ctx::root`, not the ambient current directory.
- Preview must not write `CHANGELOG.md`; all other modes use only their fixed output behavior.
