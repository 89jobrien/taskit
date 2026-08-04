//! Read-only, point-in-time view of workspace health for rendering.
//!
//! Collection only reads persisted state (the health baseline file and
//! telemetry NDJSON) — it never shells out to clippy/nextest/cargo, so it's
//! cheap enough to re-run on every dashboard tick.

use taskit_engine::ctx::Ctx;
use taskit_engine::drift::{self, DriftReport};
use taskit_engine::health::{self, HealthBaseline};
use taskit_engine::telemetry::{self, NdjsonStore, TelemetryStore};

const CI_DURATION_METRIC: &str = "ci_duration_ms";
const CI_PASSED_METRIC: &str = "ci_passed";
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
}

impl Snapshot {
    pub fn collect(ctx: &Ctx) -> Self {
        let baseline = health::load_baseline(ctx.root()).ok();
        let store = NdjsonStore::new(ctx.root());
        let records = store.load_window(DRIFT_WINDOW_DAYS).unwrap_or_default();
        Self::from_parts(baseline, &records)
    }

    fn from_parts(
        baseline: Option<HealthBaseline>,
        records: &[telemetry::TelemetryRecord],
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

        let ci_duration_drift = ci_durations.split_last().and_then(|(&current, base)| {
            if base.is_empty() {
                None
            } else {
                Some(drift::analyze(base, current))
            }
        });
        let last_ci_passed = ci_passed.last().map(|&v| v >= 1.0);
        let ci_duration_history = ci_durations
            .iter()
            .rev()
            .take(SPARKLINE_POINTS)
            .rev()
            .map(|&v| v.round() as u64)
            .collect();

        Self {
            refreshed_at: now_hms(),
            baseline,
            ci_run_count: ci_passed.len(),
            last_ci_passed,
            ci_duration_drift,
            ci_duration_history,
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
    use telemetry::{MetricPoint, TelemetryRecord};

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
        let snapshot = Snapshot::from_parts(None, &[]);
        assert!(snapshot.baseline.is_none());
        assert_eq!(snapshot.ci_run_count, 0);
        assert!(snapshot.last_ci_passed.is_none());
        assert!(snapshot.ci_duration_drift.is_none());
    }

    #[test]
    fn single_record_has_no_drift_baseline_yet() {
        let records = vec![record(
            "t1",
            &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
        )];
        let snapshot = Snapshot::from_parts(None, &records);
        assert_eq!(snapshot.ci_run_count, 1);
        assert_eq!(snapshot.last_ci_passed, Some(true));
        assert!(snapshot.ci_duration_drift.is_none());
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
        let snapshot = Snapshot::from_parts(None, &records);
        assert_eq!(snapshot.ci_run_count, 3);
        assert_eq!(snapshot.last_ci_passed, Some(false));
        let drift = snapshot
            .ci_duration_drift
            .expect("drift should be computed");
        assert!(drift.regressed, "500 vs baseline of 100s should regress");
        assert_eq!(snapshot.ci_duration_history, vec![100, 100, 500]);
    }

    #[test]
    fn duration_history_caps_at_sparkline_points() {
        let records: Vec<TelemetryRecord> = (0..(SPARKLINE_POINTS + 10))
            .map(|i| record("t", &[(CI_DURATION_METRIC, i as f64)]))
            .collect();
        let snapshot = Snapshot::from_parts(None, &records);
        assert_eq!(snapshot.ci_duration_history.len(), SPARKLINE_POINTS);
        // Oldest-first, capped to the most recent SPARKLINE_POINTS readings.
        assert_eq!(snapshot.ci_duration_history.first(), Some(&10));
        assert_eq!(
            snapshot.ci_duration_history.last(),
            Some(&((SPARKLINE_POINTS + 9) as u64))
        );
    }
}
