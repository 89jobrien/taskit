# Design: Quarantine CI Automation

## Goal

Persist CI-discovered quarantine changes through a reviewable bot pull request, with a patch artifact fallback when repository mutation is unavailable.

## Approved Approach

Apply quarantine decisions in-memory for the current run, create or update one manifest-only bot PR, never push the tested branch directly, and always preserve a fallback patch.

## Context Map

### Files to Modify

| File                                  | Purpose                                | Changes Needed                                                                           |
| ------------------------------------- | -------------------------------------- | ---------------------------------------------------------------------------------------- |
| `.github/workflows/quarantine.yml`    | New trusted automation consumer        | Validate CI artifacts and create or update the bot PR without executing contributor code |
| `.github/workflows/ci.yml`            | Main blocking CI and artifact producer | Invoke quarantine-aware tests and always upload the proposal, evidence, and run metadata |
| `.taskit/quarantine.toml`             | Checked-in state                       | Receive deterministic bot updates                                                        |
| `src/main.rs`                         | CLI integration                        | Add singular `taskit trend` and quarantine status output wiring                          |
| `crates/taskit-engine/src/command.rs` | Engine dispatch                        | Dispatch typed trend analysis and quarantine-aware test results                          |
| `taskit-protocol.lock`                | Engine command surface lock            | Record the intentional `command.rs` change                                               |

### Dependencies

The workflow depends on quarantine-aware test execution producing a deterministic proposed manifest, evidence JSON, and metadata containing the source workflow run ID, repository, event, and head SHA. It must not expose write credentials while executing untrusted fork code.

### Test Coverage

Validate workflow syntax, exercise no-change and changed-manifest paths, test PR deduplication, and verify artifact upload when permissions or push operations fail.

### Reference Patterns

Follow GitHub repository resolution in `crates/taskit-engine/src/release/gh.rs`, artifact handling in `nightly.yml`, and issue automation safeguards in `todo_sync.rs`.

## Crate Ownership

This stage owns the root CLI adapter, engine command dispatch, and GitHub Actions integration. Test telemetry and quarantine domain APIs remain owned by their preceding three-crate designs.

## Public API

Add one engine command adapter; quarantine status remains part of `taskit test run` output rather than a separate command:

```rust
pub struct Trend {
    pub window_days: u64,
}

impl Command for Trend {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError>;
}
```

The workflow relies on these CLI contracts:

```text
taskit test run
taskit trend --window 14
```

## Data Flow

1. Normal CI runs quarantine-aware tests with read-only repository permissions and always uploads a `quarantine-proposal` artifact containing the proposed manifest, patch, evidence, and source metadata.
2. `quarantine.yml` triggers through `workflow_run` after CI completes and selects that exact source run ID; it rejects artifacts whose repository, event, or head SHA do not match the workflow payload.
3. The trusted consumer checks out the default branch, never the contributor ref, and validates the proposed manifest with the default-branch taskit binary.
4. It accepts proposals only when `head_repository.full_name` equals the base repository and the changed-file set relative to the default branch contains only `.taskit/quarantine.toml`.
5. It reuses the branch of an existing open quarantine PR, or creates `automation/test-quarantine` from the default branch when none exists; updates use ordinary merge/fast-forward history and normal pushes, never force pushes.
6. It opens or refreshes one bot PR containing evidence, expiry, and recovery criteria.
7. The proposal patch and evidence remain attached to the source CI run regardless of PR success; mutation failures additionally fail the trusted workflow visibly.

## Security Boundaries

- Untrusted fork code never executes in a job holding `contents: write` or `pull-requests: write`.
- The mutation job consumes an exact run-ID artifact only after repository, event, head SHA, and manifest validation.
- Bot-triggered workflow loops are prevented by branch and actor conditions.
- The changed-file set is verified before commit so the bot cannot publish unrelated modifications.

## Out of Scope

- Direct pushes to contributor or protected branches.
- Automatic PR merge.
- External notification systems.
- Running repository code under `pull_request_target` with write credentials.

## Risk

- [ ] Breaking API changes: additive public engine `Trend` command and additive CLI command `taskit trend`; no existing API break.
- [ ] New external dependency: no; use Git and `gh` already available on GitHub runners.
- [ ] Workflow permissions: a dedicated trusted mutation job requires minimal contents and pull-request write scopes.
- [ ] Signing: bot commits may require a repository-approved signing strategy if branch policy enforces signed commits.
- [ ] Protocol lock update: yes, because `crates/taskit-engine/src/command.rs` is tracked.
