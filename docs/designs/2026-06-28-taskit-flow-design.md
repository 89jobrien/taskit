# Design: taskit flow

## Goal

Automate the `main -> staging -> release -> main` git branching lifecycle
with configurable branch names, automatic merges, and branch validation
guards.

## Approved Approach

Single `taskit flow` subcommand family in taskit-engine with four
operations: `status`, `promote`, `finish`, `guard`. Branch names
configurable via `[flow]` section in `taskit.toml`.

## Crate Ownership

- **Owner crate**: `taskit-engine` -- all flow logic lives in
  `flow.rs`, same pattern as other engine command modules
- **Affected crates**:
  - `taskit-types` -- new `FlowConfig` type in `config.rs`, new
    `FlowError` enum in `error.rs`
  - `taskit` (bin) -- new `Flow` variant in CLI `Cmd` enum

## Public API

### Types (taskit-types)

```rust
/// [flow] section in taskit.toml
#[derive(Debug, Deserialize)]
pub struct FlowConfig {
    pub main: Option<String>,      // default: "main"
    pub staging: Option<String>,   // default: "staging"
    pub release: Option<String>,   // default: "release"
}

#[derive(Debug, Error, Diagnostic)]
pub enum FlowError {
    #[error("not on expected branch: expected '{expected}', got '{actual}'")]
    #[diagnostic(
        code(taskit::flow::wrong_branch),
        help("switch to '{expected}' before running this command")
    )]
    WrongBranch {
        expected: String,
        actual: String,
    },

    #[error("branch '{branch}' is protected -- direct commits are blocked")]
    #[diagnostic(
        code(taskit::flow::protected),
        help("commit to '{staging}' and use `taskit flow promote`")
    )]
    ProtectedBranch {
        branch: String,
        staging: String,
    },

    #[error("branch '{branch}' does not exist")]
    #[diagnostic(
        code(taskit::flow::missing_branch),
        help("create it with: git branch {branch}")
    )]
    MissingBranch {
        branch: String,
    },

    #[error("branch '{branch}' has uncommitted changes")]
    #[diagnostic(
        code(taskit::flow::dirty),
        help("commit or stash changes before flow operations")
    )]
    DirtyWorktree {
        branch: String,
    },

    #[error("merge failed: {reason}")]
    #[diagnostic(code(taskit::flow::merge_failed))]
    MergeFailed {
        reason: String,
    },
}
```

### Functions (taskit-engine)

```rust
/// Show current branch position relative to flow branches.
/// Prints ahead/behind counts for each branch pair.
pub fn status(sh: &Shell, flow: &FlowConfig) -> Result<(), TaskitError>;

/// Merge staging into release. Must be run from staging branch.
/// Creates a --no-ff merge commit.
pub fn promote(sh: &Shell, flow: &FlowConfig) -> Result<(), TaskitError>;

/// Merge release back into main. Must be run from release branch.
/// Creates a --no-ff merge commit, then merges main into staging
/// to keep staging up to date.
pub fn finish(sh: &Shell, flow: &FlowConfig) -> Result<(), TaskitError>;

/// Validate that the current branch is allowed for commits.
/// Blocks if on main or release (protected branches).
/// Intended for pre-commit hook integration.
pub fn guard(sh: &Shell, flow: &FlowConfig) -> Result<(), TaskitError>;
```

## Data Flow

1. `main.rs` dispatches `Cmd::Flow { sub }` to `flow::{status,
promote, finish, guard}`
2. Each function reads current branch via `git branch --show-current`
3. `promote`: validates on staging, checks clean worktree, runs
   `git checkout release && git merge --no-ff staging && git checkout
staging`
4. `finish`: validates on release, runs `git checkout main && git
merge --no-ff release && git checkout staging && git merge --no-ff
main && git checkout staging`
5. `guard`: reads current branch, returns `FlowError::ProtectedBranch`
   if on main or release
6. `status`: runs `git rev-list --count` for ahead/behind between
   each pair

## Hexagonal Boundaries

- **Port**: none needed -- git operations are via xshell `cmd!` macro,
  consistent with all other engine modules
- **Adapter**: `flow.rs` in taskit-engine, same pattern as `hooks.rs`

Git is not abstracted behind a trait because:

- Every other engine module (affected, hooks, protocol) calls git
  directly via xshell
- There is exactly one implementation (git CLI)
- Adding a trait would violate YAGNI

## Config Integration

`taskit.toml` gains an optional `[flow]` section:

```toml
[flow]
main = "main"        # default
staging = "staging"  # default
release = "release"  # default
```

All fields optional. `FlowConfig` provides defaults via an impl:

```rust
impl FlowConfig {
    pub fn main_branch(&self) -> &str;
    pub fn staging_branch(&self) -> &str;
    pub fn release_branch(&self) -> &str;
}

impl Default for FlowConfig {
    // main="main", staging="staging", release="release"
}
```

## Out of Scope

- Feature branches (user manages those manually)
- Remote push (user pushes after flow operations)
- Release publishing (handled by `cargo rail release`)
- Branch creation (`git branch` is sufficient)

## Risk

- [ ] Breaking API changes: yes -- new `FlowError` variant added to
      `TaskitError` enum, new `flow` field on `Config` struct. Both are
      additive (new enum variant, new `Option` field).
- [ ] New external dependency: no
- [ ] Feature flag required: no
