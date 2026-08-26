//! Historical drift analysis over [`telemetry`](crate::telemetry) records.
//!
//! Compares the most recent reading of a metric against the distribution of
//! prior readings within a time window, flagging a regression when the
//! latest value clears the window's 95th percentile by more than 20%.

use taskit_types::error::TaskitError;

use crate::ctx::Ctx;
use crate::telemetry::{NdjsonStore, TelemetryStore};

const REGRESSION_FACTOR: f64 = 1.20;

/// Pure result of comparing a metric's latest reading against its baseline
/// window. Exposed separately from [`run`] so callers that need the numbers
/// without CLI side effects (e.g. `taskit-tui`) can reuse the same analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftReport {
    /// Arithmetic mean of baseline samples.
    pub baseline_mean: f64,
    /// 95th percentile of baseline samples.
    pub baseline_p95: f64,
    /// Most recent metric value.
    pub current: f64,
    /// Percent delta of current vs baseline mean.
    pub delta_pct: f64,
    /// True when current exceeds regression threshold.
    pub regressed: bool,
}

/// Compare `current` against `baseline` readings using the same regression
/// rule as `taskit health drift`: flagged when `current` clears the
/// baseline's p95 by more than `REGRESSION_FACTOR`.
pub fn analyze(baseline: &[f64], current: f64) -> DriftReport {
    let baseline_mean = mean(baseline);
    let baseline_p95 = percentile(baseline, 0.95);
    let delta_pct = if baseline_mean == 0.0 {
        0.0
    } else {
        (current - baseline_mean) / baseline_mean * 100.0
    };
    let regressed = baseline_p95 > 0.0 && current > baseline_p95 * REGRESSION_FACTOR;
    DriftReport {
        baseline_mean,
        baseline_p95,
        current,
        delta_pct,
        regressed,
    }
}

/// Load the historical readings for `metric` within `window_days` from a
/// [`TelemetryStore`], returning `(baseline, current)` where `current` is the
/// most recent reading and `baseline` is everything before it. `None` if
/// there are fewer than two readings in the window.
///
/// Depends on the `TelemetryStore` port, not a concrete backend — swapping
/// storage (NDJSON, SQLite, ...) never requires touching this function.
pub fn load_metric_history(
    store: &dyn TelemetryStore,
    metric: &str,
    window_days: u64,
) -> Result<Option<(Vec<f64>, f64)>, TaskitError> {
    let records = store.load_window(window_days)?;
    let points: Vec<f64> = records
        .iter()
        .flat_map(|r| r.metrics.iter())
        .filter(|m| m.name == metric)
        .map(|m| m.value)
        .collect();
    Ok(match points.split_last() {
        Some((&current, baseline)) if !baseline.is_empty() => Some((baseline.to_vec(), current)),
        _ => None,
    })
}

/// Run `taskit drift --metric <name> --window <days>`.
pub fn run(ctx: &Ctx, metric: &str, window_days: u64) -> Result<(), TaskitError> {
    let store = NdjsonStore::new(ctx.root());
    match load_metric_history(&store, metric, window_days)? {
        None => {
            taskit_output::taskit_progress!(
                "Not enough telemetry for {metric:?} in the last {window_days}d to compare yet."
            );
            Ok(())
        }
        Some((baseline, current)) => report(metric, window_days, &baseline, current),
    }
}

fn report(
    metric: &str,
    window_days: u64,
    baseline: &[f64],
    current: f64,
) -> Result<(), TaskitError> {
    let d = analyze(baseline, current);

    taskit_output::taskit_progress!(
        "Drift Analysis: {metric} (current vs {window_days}d baseline, n={})",
        baseline.len()
    );
    taskit_output::taskit_progress!("{}", "-".repeat(50));
    taskit_output::taskit_progress!("baseline mean:  {:.2}", d.baseline_mean);
    taskit_output::taskit_progress!("baseline p95:   {:.2}", d.baseline_p95);
    taskit_output::taskit_progress!(
        "current:        {:.2} ({:+.1}% vs mean)",
        d.current,
        d.delta_pct
    );
    taskit_output::taskit_progress!("{}", "-".repeat(50));

    if d.regressed {
        taskit_output::taskit_err!(
            "DRIFT: current exceeds baseline p95 by more than {:.0}%",
            (REGRESSION_FACTOR - 1.0) * 100.0
        );
        Err(TaskitError::other(format!(
            "drift detected for {metric}: current {:.2} > baseline p95 {:.2} * {REGRESSION_FACTOR}",
            d.current, d.baseline_p95
        )))
    } else {
        taskit_output::taskit_ok!("STABLE");
        Ok(())
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Nearest-rank percentile (`p` in `[0.0, 1.0]`) over `values`.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{MetricPoint, TelemetryRecord};

    /// In-memory `TelemetryStore` test double — the payoff of depending on
    /// the port rather than `NdjsonStore` directly: no tempdir, no real I/O.
    struct FakeStore {
        records: Vec<TelemetryRecord>,
    }

    impl TelemetryStore for FakeStore {
        fn record(&self, _entry: TelemetryRecord) -> Result<(), TaskitError> {
            unimplemented!("drift only reads telemetry")
        }

        fn load_window(&self, _window_days: u64) -> Result<Vec<TelemetryRecord>, TaskitError> {
            Ok(self.records.clone())
        }
    }

    fn fake_record(metric: &str, value: f64) -> TelemetryRecord {
        TelemetryRecord {
            timestamp: "t".into(),
            git_sha: None,
            metrics: vec![MetricPoint {
                name: metric.into(),
                value,
            }],
        }
    }

    #[test]
    fn load_metric_history_via_fake_store() {
        let store = FakeStore {
            records: vec![
                fake_record("ci_duration_ms", 100.0),
                fake_record("ci_duration_ms", 500.0),
            ],
        };
        let (baseline, current) = load_metric_history(&store, "ci_duration_ms", 7)
            .unwrap()
            .expect("two readings should yield a baseline + current");
        assert_eq!(baseline, vec![100.0]);
        assert_eq!(current, 500.0);
    }

    #[test]
    fn load_metric_history_single_reading_is_none() {
        let store = FakeStore {
            records: vec![fake_record("ci_duration_ms", 100.0)],
        };
        assert!(
            load_metric_history(&store, "ci_duration_ms", 7)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn mean_of_empty_is_zero() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn mean_basic() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), 2.0);
    }

    #[test]
    fn percentile_p95_of_sorted_run() {
        let values: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        assert_eq!(percentile(&values, 0.95), 95.0);
    }

    #[test]
    fn percentile_empty_is_zero() {
        assert_eq!(percentile(&[], 0.95), 0.0);
    }

    #[test]
    fn report_flags_regression_above_threshold() {
        let baseline = vec![100.0, 100.0, 100.0, 100.0];
        assert!(report("d", 7, &baseline, 130.0).is_err());
    }

    #[test]
    fn report_passes_within_threshold() {
        let baseline = vec![100.0, 100.0, 100.0, 100.0];
        assert!(report("d", 7, &baseline, 109.0).is_ok());
    }

    #[test]
    fn report_zero_baseline_never_regresses() {
        let baseline = vec![0.0, 0.0, 0.0];
        assert!(report("d", 7, &baseline, 5.0).is_ok());
    }

    #[test]
    fn run_with_no_records_is_ok() {
        let ctx = Ctx::test();
        assert!(run(&ctx, "nonexistent_metric", 7).is_ok());
    }
}
