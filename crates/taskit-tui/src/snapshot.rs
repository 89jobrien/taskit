//! Read-only, point-in-time view of workspace health for rendering.
//!
//! Collection only reads persisted state (the health baseline file,
//! telemetry NDJSON, flow state file, and git/protocol-surface reads that
//! never shell out to clippy/nextest/cargo) — so it's cheap enough to
//! re-run on every dashboard tick.

use taskit_engine::ctx::Ctx;
use taskit_engine::drift::{self, DriftReport};
use taskit_engine::flow::{self, FlowStatusReport};
use taskit_engine::flow_state_store;
use taskit_engine::health::{self, HealthBaseline};
use taskit_engine::protocol::drift::{self as protocol_drift, ProtocolDriftStatus};
use taskit_engine::telemetry::{NdjsonStore, TelemetryRecord, TelemetryStore};
use taskit_types::config::ConflictResolverKind;
use taskit_types::flow_state::FlowState;

const CI_DURATION_METRIC: &str = "ci_duration_ms";
const CI_PASSED_METRIC: &str = "ci_passed";
const FLOW_AUTO_DURATION_METRIC: &str = "flow_auto_duration_ms";
const FLOW_AUTO_RESULT_METRIC: &str = "flow_auto_result";
const FLOW_AUTO_CONFLICTS_METRIC: &str = "flow_auto_conflicts";
const DRIFT_WINDOW_DAYS: u64 = 7;
const SPARKLINE_POINTS: usize = 30;

pub struct Snapshot {
    pub refreshed_at: String,
    pub baseline: Option<HealthBaseline>,
    pub ci_run_count: usize,
    pub last_ci_passed: Option<bool>,
    pub ci_duration_drift: Option<DriftReport>,
    /// Raw `ci_duration_ms` readings, oldest first, capped to the last
    /// [`SPARKLINE_POINTS`] — feeds the dashboard's `Sparkline` widget.
    pub ci_duration_history: Vec<u64>,
    /// Raw `ci_passed` readings (0.0/1.0), oldest first, capped to the last
    /// [`SPARKLINE_POINTS`] — feeds the pass/fail trend strip.
    pub ci_passed_history: Vec<u64>,
    /// Full telemetry records in the drift window, oldest first — feeds the
    /// scrollable CI History tab.
    pub records: Vec<TelemetryRecord>,
    /// Git-flow pipeline hop status (main→develop→staging→release→main).
    pub flow_status: Option<FlowStatusReport>,
    /// Resumable `flow auto` state, if a run was interrupted mid-pipeline.
    pub flow_state: Option<FlowState>,
    pub flow_conflict_resolver: ConflictResolverKind,
    /// Raw `flow_auto_duration_ms` readings, oldest first, capped like
    /// `ci_duration_history`.
    pub flow_auto_duration_history: Vec<u64>,
    /// Raw `flow_auto_result` readings (0/1), oldest first, capped like
    /// `ci_passed_history`.
    pub flow_auto_result_history: Vec<u64>,
    pub flow_auto_conflicts_last: Option<u64>,
    pub protocol_drift: Option<ProtocolDriftStatus>,
}

impl Snapshot {
    pub fn collect(ctx: &Ctx) -> Self {
        let baseline = health::load_baseline(ctx.root()).ok();
        let store = NdjsonStore::new(ctx.root());
        let records = store.load_window(DRIFT_WINDOW_DAYS).unwrap_or_default();

        let flow_config = ctx.config.flow.clone().unwrap_or_default();
        let flow_status = flow::status_report(ctx, &flow_config).ok();
        let flow_state = flow_state_store::load(ctx.root());
        let flow_conflict_resolver = flow_config.conflict_resolver;

        let drift_status = protocol_drift::check(ctx).ok();

        Self::from_parts(
            baseline,
            &records,
            flow_status,
            flow_state,
            flow_conflict_resolver,
            drift_status,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        baseline: Option<HealthBaseline>,
        records: &[TelemetryRecord],
        flow_status: Option<FlowStatusReport>,
        flow_state: Option<FlowState>,
        flow_conflict_resolver: ConflictResolverKind,
        protocol_drift: Option<ProtocolDriftStatus>,
    ) -> Self {
        let metric_values = |name: &str| -> Vec<f64> {
            records
                .iter()
                .flat_map(|r| r.metrics.iter())
                .filter(|m| m.name == name)
                .map(|m| m.value)
                .collect()
        };

        let ci_durations = metric_values(CI_DURATION_METRIC);
        let ci_passed = metric_values(CI_PASSED_METRIC);
        let flow_auto_durations = metric_values(FLOW_AUTO_DURATION_METRIC);
        let flow_auto_results = metric_values(FLOW_AUTO_RESULT_METRIC);
        let flow_auto_conflicts = metric_values(FLOW_AUTO_CONFLICTS_METRIC);

        let ci_duration_drift = ci_durations.split_last().and_then(|(&current, base)| {
            if base.is_empty() {
                None
            } else {
                Some(drift::analyze(base, current))
            }
        });
        let last_ci_passed = ci_passed.last().map(|&v| v >= 1.0);
        let recent = |values: &[f64]| -> Vec<u64> {
            values
                .iter()
                .rev()
                .take(SPARKLINE_POINTS)
                .rev()
                .map(|&v| v.round() as u64)
                .collect()
        };
        let ci_duration_history = recent(&ci_durations);
        let ci_passed_history = recent(&ci_passed);
        let flow_auto_duration_history = recent(&flow_auto_durations);
        let flow_auto_result_history = recent(&flow_auto_results);
        let flow_auto_conflicts_last = flow_auto_conflicts.last().map(|&v| v.round() as u64);

        Self {
            refreshed_at: now_hms(),
            baseline,
            ci_run_count: ci_passed.len(),
            last_ci_passed,
            ci_duration_drift,
            ci_duration_history,
            ci_passed_history,
            records: records.to_vec(),
            flow_status,
            flow_state,
            flow_conflict_resolver,
            flow_auto_duration_history,
            flow_auto_result_history,
            flow_auto_conflicts_last,
            protocol_drift,
        }
    }
}

fn now_hms() -> String {
    std::process::Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_engine::telemetry::MetricPoint;

    fn record(ts: &str, metrics: &[(&str, f64)]) -> TelemetryRecord {
        TelemetryRecord {
            timestamp: ts.into(),
            git_sha: None,
            metrics: metrics
                .iter()
                .map(|(name, value)| MetricPoint {
                    name: (*name).to_string(),
                    value: *value,
                })
                .collect(),
        }
    }

    #[test]
    fn no_records_yields_empty_snapshot() {
        let snapshot =
            Snapshot::from_parts(None, &[], None, None, ConflictResolverKind::default(), None);
        assert!(snapshot.baseline.is_none());
        assert_eq!(snapshot.ci_run_count, 0);
        assert!(snapshot.last_ci_passed.is_none());
        assert!(snapshot.ci_duration_drift.is_none());
        assert!(snapshot.records.is_empty());
        assert!(snapshot.flow_status.is_none());
        assert!(snapshot.flow_state.is_none());
        assert!(snapshot.protocol_drift.is_none());
    }

    #[test]
    fn single_record_has_no_drift_baseline_yet() {
        let records = vec![record(
            "t1",
            &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
        )];
        let snapshot = Snapshot::from_parts(
            None,
            &records,
            None,
            None,
            ConflictResolverKind::default(),
            None,
        );
        assert_eq!(snapshot.ci_run_count, 1);
        assert_eq!(snapshot.last_ci_passed, Some(true));
        assert!(snapshot.ci_duration_drift.is_none());
        assert_eq!(snapshot.records.len(), 1);
    }

    #[test]
    fn multiple_records_compute_drift_against_prior_readings() {
        let records = vec![
            record(
                "t1",
                &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
            ),
            record(
                "t2",
                &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
            ),
            record(
                "t3",
                &[(CI_DURATION_METRIC, 500.0), (CI_PASSED_METRIC, 0.0)],
            ),
        ];
        let snapshot = Snapshot::from_parts(
            None,
            &records,
            None,
            None,
            ConflictResolverKind::default(),
            None,
        );
        assert_eq!(snapshot.ci_run_count, 3);
        assert_eq!(snapshot.last_ci_passed, Some(false));
        let drift = snapshot
            .ci_duration_drift
            .expect("drift should be computed");
        assert!(drift.regressed, "500 vs baseline of 100s should regress");
        assert_eq!(snapshot.ci_duration_history, vec![100, 100, 500]);
        assert_eq!(snapshot.ci_passed_history, vec![1, 1, 0]);
    }

    #[test]
    fn duration_history_caps_at_sparkline_points() {
        let records: Vec<TelemetryRecord> = (0..(SPARKLINE_POINTS + 10))
            .map(|i| record("t", &[(CI_DURATION_METRIC, i as f64)]))
            .collect();
        let snapshot = Snapshot::from_parts(
            None,
            &records,
            None,
            None,
            ConflictResolverKind::default(),
            None,
        );
        assert_eq!(snapshot.ci_duration_history.len(), SPARKLINE_POINTS);
        // Oldest-first, capped to the most recent SPARKLINE_POINTS readings.
        assert_eq!(snapshot.ci_duration_history.first(), Some(&10));
        assert_eq!(
            snapshot.ci_duration_history.last(),
            Some(&((SPARKLINE_POINTS + 9) as u64))
        );
        assert_eq!(snapshot.records.len(), SPARKLINE_POINTS + 10);
    }

    #[test]
    fn flow_auto_history_derived_same_way_as_ci_history() {
        let records = vec![
            record(
                "t1",
                &[
                    ("flow_auto_duration_ms", 1000.0),
                    ("flow_auto_result", 1.0),
                    ("flow_auto_conflicts", 2.0),
                ],
            ),
            record(
                "t2",
                &[
                    ("flow_auto_duration_ms", 2000.0),
                    ("flow_auto_result", 0.0),
                    ("flow_auto_conflicts", 0.0),
                ],
            ),
        ];
        let snapshot = Snapshot::from_parts(
            None,
            &records,
            None,
            None,
            ConflictResolverKind::default(),
            None,
        );
        assert_eq!(snapshot.flow_auto_duration_history, vec![1000, 2000]);
        assert_eq!(snapshot.flow_auto_result_history, vec![1, 0]);
        assert_eq!(snapshot.flow_auto_conflicts_last, Some(0));
    }
}
