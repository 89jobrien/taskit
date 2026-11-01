# Design: Automatic Flaky-Test Quarantine

## Goal

Automatically isolate empirically flaky tests from the blocking lane while preserving evidence, expiry, recovery checks, and a reviewable checked-in manifest.

## Approved Approach

Use conservative deterministic classification, a checked-in `.taskit/quarantine.toml`, targeted retries, and a separate non-blocking quarantine lane.

## Context Map

### Files to Modify

| File                                                | Purpose                           | Changes Needed                                                                 |
| --------------------------------------------------- | --------------------------------- | ------------------------------------------------------------------------------ |
| `crates/taskit-types/src/quarantine.rs`             | New manifest and policy contracts | Define versioned entries, evidence, and thresholds                             |
| `crates/taskit-types/src/lib.rs`                    | Contract exports                  | Export quarantine types                                                        |
| `crates/taskit-core/src/quarantine.rs`              | New repository port               | Define load and deterministic save operations                                  |
| `crates/taskit-core/src/lib.rs`                     | Port exports                      | Export the quarantine repository                                               |
| `crates/taskit-engine/src/quarantine/mod.rs`        | Quarantine orchestration          | Load state, classify, apply lifecycle, and report decisions                    |
| `crates/taskit-engine/src/quarantine/classifier.rs` | Pure classification               | Implement approved thresholds and caps                                         |
| `crates/taskit-engine/src/quarantine/manifest.rs`   | TOML adapter                      | Validate and atomically persist the checked-in manifest                        |
| `crates/taskit-engine/src/testing/run.rs`           | Test lanes and retries            | Exclude active entries, retry candidates, and run quarantined tests separately |
| `.gitignore`                                        | Local telemetry exclusions        | Keep `.taskit/quarantine.toml` tracked while ignoring other `.taskit` state    |
| `.taskit/quarantine.toml`                           | Checked-in quarantine state       | Initialize schema version with no entries                                      |

### Dependencies

This design consumes `TestEvent`, `TestId`, `TestOutcome`, and `TestHistoryStore` from the test telemetry design. Normal test execution remains the integration point; no TUI dependency is introduced.

### Test Coverage

Use table-driven classifier tests for thresholds, expiry, recovery, caps, malformed manifests, and deterministic failures. Add nextest fixtures for blocking, retry, and quarantine lanes plus manifest round-trip tests.

### Reference Patterns

Follow protocol lock validation in `protocol/drift.rs`, atomic state behavior in `flow_state_store.rs`, and pipeline gate behavior in `step.rs`.

## Crate Ownership

- **Contract crate**: `taskit-types` — owns policy and serialized manifest types.
- **Port crate**: `taskit-core` — owns quarantine persistence behavior.
- **Orchestration crate**: `taskit-engine` — owns classification, manifest adapter, retries, and lane execution.

## Public API

### Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineEvidence {
    pub observations: usize,
    pub passes: usize,
    pub failures: usize,
    pub distinct_runs: usize,
    pub last_git_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineEntry {
    pub test_id: TestId,
    pub reason: String,
    pub created_at: String,
    pub expires_at: String,
    pub consecutive_passes: usize,
    pub evidence: QuarantineEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineManifest {
    pub schema_version: u32,
    pub entries: Vec<QuarantineEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuarantinePolicy {
    pub window_days: u64,
    pub minimum_observations: usize,
    pub minimum_distinct_runs: usize,
    pub minimum_passes: usize,
    pub minimum_failures: usize,
    pub targeted_retries: usize,
    pub expiry_days: u64,
    pub recovery_passes: usize,
    pub maximum_tests: usize,
    pub maximum_suite_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineDecision {
    Add(QuarantineEntry),
    Renew(QuarantineEntry),
    Remove(TestId),
    Keep,
}
```

### Traits

```rust
pub trait QuarantineRepository {
    fn load(&self) -> Result<QuarantineManifest, TaskitError>;
    fn save(&self, manifest: &QuarantineManifest) -> Result<(), TaskitError>;
}
```

### Functions

```rust
pub fn classify(
    test_id: &TestId,
    events: &[TestEvent],
    current: Option<&QuarantineEntry>,
    policy: &QuarantinePolicy,
    now: &str,
) -> QuarantineDecision;

pub fn apply_decisions(
    manifest: &QuarantineManifest,
    decisions: &[QuarantineDecision],
    suite_size: usize,
    policy: &QuarantinePolicy,
) -> Result<QuarantineManifest, TaskitError>;
```

## Classification Rules

Defaults are a 14-day window, six observations from the same normalized environment, two distinct runs, two passes, two failures, three targeted retries, seven-day expiry, five recovery passes, and a cap of ten tests or two percent of the suite. Per-test `classify` produces evidence decisions; suite-wide `apply_decisions` applies removals and renewals first, then ranks additions by distinct runs descending, observations descending, and `TestId` ascending until both caps are reached. Classification never applies to compilation, setup, infrastructure, or suite-level failures.

## Data Flow

1. Load and strictly validate the manifest; malformed state fails the command before nextest starts, so no test is silently excluded.
2. Build nextest filters through a typed expression builder keyed by package, binary, and exact fully qualified test name; escape every value and reject identities that cannot be represented exactly.
3. Run non-quarantined tests as the blocking lane and record all attempts.
4. Target only failing test IDs for retries and append each attempt to history.
5. Classify IDs from same-environment bounded history and retry evidence, enforcing absolute and fractional caps.
6. When mixed outcomes satisfy the approved policy, remove that test's initial failure from the final blocking outcome for the current run; deterministic or insufficiently observed failures remain blocking.
7. Apply additions, renewals, removals, and expiry deterministically to the manifest.
8. Run active quarantines separately as non-blocking tests so recovery remains observable.
9. Emit every decision and quarantined failure in human, JSON, and SARIF output.

Quarantine decisions reuse `DiagnosticRecord`: `TQ001` is a warning for additions or renewals, `TQ002` is a warning for an active quarantined failure, and `TQ003` is a note for recovery removal. Deterministic test failures retain the existing error-level `TE001` or `TE002` records.

## Hexagonal Boundaries

- **Port**: `QuarantineRepository` in `taskit-core`.
- **Adapter**: `TomlQuarantineRepository` in `taskit-engine::quarantine::manifest`.
- **Pure policy**: `classify` has no filesystem, clock, Git, or subprocess access.

The repository keeps `.taskit/quarantine.toml` tracked by changing the ignore rule from `.taskit/` to `.taskit/*` and adding `!.taskit/quarantine.toml`; all telemetry partitions remain ignored.

## Out of Scope

- Machine-learning classification.
- Permanent exclusions.
- Direct branch pushes.
- Database-backed manifests.
- TUI mutation controls.

## Risk

- [ ] Breaking API changes: no.
- [ ] Serialization format changes: new versioned manifest.
- [ ] New external dependency: no; reuse existing TOML support.
- [ ] Safety risk: automatic exclusion is bounded, expiring, observable, and fail-closed.
