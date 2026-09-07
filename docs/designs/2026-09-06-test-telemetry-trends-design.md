# Design: Test Telemetry and Trends

## Goal

Capture stable per-attempt test history and expose deterministic reliability and duration trends through the CLI.

## Approved Approach

Extend the existing nextest JSON and NDJSON patterns without adding a database, record all attempts rather than failures only, and deliver CLI trends before TUI integration.

## Context Map

### Files to Modify

| File                                      | Purpose                             | Changes Needed                                                                                                    |
| ----------------------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `crates/taskit-types/src/test_event.rs`   | New serialized test-event contracts | Define stable identity, outcome, run metadata, and event envelope                                                 |
| `crates/taskit-types/src/lib.rs`          | Contract exports                    | Export test-event types                                                                                           |
| `crates/taskit-core/src/test_history.rs`  | New storage port                    | Define append and bounded-history operations                                                                      |
| `crates/taskit-core/src/lib.rs`           | Port exports                        | Export the history port                                                                                           |
| `crates/taskit-engine/src/testing/run.rs` | Nextest execution and parsing       | Consume `libtest-json-plus` message-format version 1 and preserve retries, duration, package, and target identity |
| `crates/taskit-engine/src/telemetry.rs`   | Existing NDJSON adapter pattern     | Add a versioned test-history adapter under `.taskit/telemetry`                                                    |
| `crates/taskit-engine/src/trends.rs`      | New trend analysis                  | Aggregate reliability and duration by stable test ID                                                              |

### Dependencies

`taskit-engine` consumes contracts from `taskit-types` and the port from `taskit-core`. Existing aggregate `TelemetryRecord` remains readable and unchanged; the TUI continues consuming it until a later design. Root CLI wiring is specified separately in the quarantine CI automation design so this design remains within three crate boundaries.

### Test Coverage

Add `libtest-json-plus` message-format version 1 fixtures containing passes, failures, retries, duration, package, binary, and target data. Use an in-memory `TestHistoryStore` to test aggregation independently from NDJSON.

### Reference Patterns

Follow `TelemetryStore`/`NdjsonStore` in `telemetry.rs`, fake-store tests in `drift.rs`, and diagnostic parsing in `testing/run.rs`.

## Crate Ownership

- **Contract crate**: `taskit-types` — owns stable serialized event types.
- **Port crate**: `taskit-core` — owns storage behavior required by trend analysis.
- **Orchestration crate**: `taskit-engine` — owns nextest parsing, the NDJSON adapter, aggregation, and command behavior.

## Public API

### Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestId {
    pub package: String,
    pub target_kind: String,
    pub target_name: String,
    pub binary: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    Passed,
    Failed,
    TimedOut,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestEvent {
    pub schema_version: u32,
    pub run_id: String,
    pub test_id: TestId,
    pub outcome: TestOutcome,
    pub duration_ms: u64,
    pub attempt: u32,
    pub git_sha: Option<String>,
    pub environment: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunMetadata {
    pub run_id: String,
    pub git_sha: Option<String>,
    pub environment: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestTrend {
    pub test_id: TestId,
    pub observations: usize,
    pub passes: usize,
    pub failures: usize,
    pub failure_rate: f64,
    pub mean_duration_ms: f64,
    pub p95_duration_ms: u64,
    pub last_outcome: TestOutcome,
}
```

### Traits

```rust
pub trait TestHistoryStore {
    fn append(&self, events: &[TestEvent]) -> Result<(), TaskitError>;
    fn load_window(&self, window_days: u64) -> Result<Vec<TestEvent>, TaskitError>;
}
```

### Functions

```rust
pub fn analyze_test_trends(events: &[TestEvent]) -> Vec<TestTrend>;
pub fn parse_nextest_test_events(
    json_lines: &str,
    metadata: &TestRunMetadata,
) -> Result<Vec<TestEvent>, TaskitError>;
```

`TestTrend` and `analyze_test_trends` are owned by `taskit-engine::trends`; event and run metadata types are owned by `taskit-types`. `run_id` is generated once per invocation from the Unix timestamp in nanoseconds, process ID, and Git SHA prefix, requiring no new dependency. `environment` is an explicit normalized runner/target label, `timestamp` is UTC RFC 3339, and `git_sha` comes from the tested checkout.

## Data Flow

1. Nextest emits `libtest-json-plus` with `--message-format-version 1` for every test attempt, including package, binary, target, and execution fields.
2. The engine parser normalizes each event into a versioned `TestEvent` with stable identity and run metadata.
3. `TestHistoryStore` strictly parses and appends events to daily NDJSON partitions; malformed records return a path-and-line error rather than disappearing silently.
4. Trend analysis loads a bounded window and groups events by `TestId`.
5. Trend analysis uses arithmetic mean and nearest-rank p95, then returns typed reports for a later CLI adapter.

## Hexagonal Boundaries

- **Port**: `TestHistoryStore` in `taskit-core`.
- **Adapter**: `NdjsonTestHistoryStore` in `taskit-engine::telemetry`.

## Out of Scope

- Quarantine decisions or exclusions.
- SQLite or remote telemetry.
- TUI trend controls.
- Retrofitting aggregate telemetry into test events.

## Risk

- [ ] Breaking API changes: no; existing telemetry types remain in place.
- [ ] Serialization format changes: additive, versioned test-event stream.
- [ ] New external dependency: no.
- [ ] Protocol lock update: no in this stage; root command wiring is specified separately.
