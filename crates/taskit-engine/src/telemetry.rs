//! Telemetry storage port, for historical drift analysis.
//!
//! [`TelemetryStore`] is the port: commands that produce time-series-worthy
//! metrics (CI duration, health counts, ...) record through it, and
//! [`drift`](crate::drift) reads back a bounded window through it. The only
//! adapter today is [`NdjsonStore`], an append-only NDJSON layout under
//! `.taskit/telemetry/<YYYY>/<MM>/<DD>/history.ndjson`, partitioned by day so
//! a windowed read doesn't have to parse the whole history. Swapping to a
//! different backing store (SQLite, DuckDB, ...) means adding a new
//! `TelemetryStore` impl, not touching `drift.rs` or the CI hook.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use taskit_types::error::{TaskitError, TaskitResultExt};

use crate::ctx::Ctx;

const TELEMETRY_DIR: &str = ".taskit/telemetry";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricPoint {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryRecord {
    /// RFC 3339 timestamp (UTC).
    pub timestamp: String,
    pub git_sha: Option<String>,
    pub metrics: Vec<MetricPoint>,
}

/// Port for persisting and querying telemetry history. Consumers (`drift`,
/// `taskit-tui`) depend on this trait, not on the NDJSON layout — see
/// [`NdjsonStore`] for the only adapter today.
pub trait TelemetryStore {
    /// Persist one telemetry record.
    fn record(&self, entry: TelemetryRecord) -> Result<(), TaskitError>;
    /// Load all records from the last `window_days` days, oldest first.
    fn load_window(&self, window_days: u64) -> Result<Vec<TelemetryRecord>, TaskitError>;
}

/// Append-only NDJSON adapter, partitioned by day under
/// `<root>/.taskit/telemetry/<YYYY>/<MM>/<DD>/history.ndjson`.
pub struct NdjsonStore {
    root: PathBuf,
}

impl NdjsonStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl TelemetryStore for NdjsonStore {
    fn record(&self, entry: TelemetryRecord) -> Result<(), TaskitError> {
        append_record(&self.root, &entry)
    }

    fn load_window(&self, window_days: u64) -> Result<Vec<TelemetryRecord>, TaskitError> {
        load_window(&self.root, window_days)
    }
}

/// Append a telemetry record capturing `metrics` via the workspace's
/// [`NdjsonStore`].
///
/// No-op in dry-run mode: telemetry describes what actually ran, so a
/// dry-run pass shouldn't pollute the history.
pub fn record(ctx: &Ctx, metrics: &[(&str, f64)]) -> Result<(), TaskitError> {
    if ctx.dry_run {
        return Ok(());
    }
    let entry = TelemetryRecord {
        timestamp: now_rfc3339(),
        git_sha: command_output(ctx.root(), "git", &["rev-parse", "HEAD"]),
        metrics: metrics
            .iter()
            .map(|(name, value)| MetricPoint {
                name: (*name).to_string(),
                value: *value,
            })
            .collect(),
    };
    NdjsonStore::new(ctx.root()).record(entry)
}

/// Return the most recent reading of `metric` within the last `window_days`
/// days, or `None` if there is no such reading.
pub fn latest_metric(
    store: &dyn TelemetryStore,
    metric: &str,
    window_days: u64,
) -> Result<Option<f64>, TaskitError> {
    let records = store.load_window(window_days)?;
    Ok(records
        .iter()
        .flat_map(|r| r.metrics.iter())
        .rfind(|m| m.name == metric)
        .map(|m| m.value))
}

/// Load all telemetry records from the last `window_days` days, oldest first,
/// via the NDJSON layout rooted at `root`.
fn load_window(root: &Path, window_days: u64) -> Result<Vec<TelemetryRecord>, TaskitError> {
    let dir = root.join(TELEMETRY_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let cutoff = today_epoch_days() - window_days as i64;
    let mut records = Vec::new();
    for year_entry in read_dir_sorted(&dir)? {
        for month_entry in read_dir_sorted(&year_entry)? {
            for day_dir in read_dir_sorted(&month_entry)? {
                let Some(epoch_day) = epoch_days_from_day_dir(&day_dir) else {
                    continue;
                };
                if epoch_day < cutoff {
                    continue;
                }
                for day_file in read_dir_sorted(&day_dir)? {
                    records.extend(read_ndjson(&day_file)?);
                }
            }
        }
    }
    records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(records)
}

// -- internals -----------------------------------------------------------

fn append_record(root: &Path, entry: &TelemetryRecord) -> Result<(), TaskitError> {
    let dir = root.join(TELEMETRY_DIR).join(today_path());
    std::fs::create_dir_all(&dir)
        .err_context_with(|| format!("failed to create {}", dir.display()))?;
    let file_path = dir.join("history.ndjson");
    let line = serde_json::to_string(entry).map_err(TaskitError::other)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .err_context_with(|| format!("failed to open {}", file_path.display()))?;
    writeln!(file, "{line}").err_context_with(|| format!("failed to write {}", file_path.display()))
}

fn read_ndjson(path: &Path) -> Result<Vec<TelemetryRecord>, TaskitError> {
    let content = std::fs::read_to_string(path)
        .err_context_with(|| format!("failed to read {}", path.display()))?;
    Ok(content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<PathBuf>, TaskitError> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .err_context_with(|| format!("failed to read {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    Ok(entries)
}

/// Parse a `.../YYYY/MM/DD` directory path into days since the Unix epoch.
fn epoch_days_from_day_dir(path: &Path) -> Option<i64> {
    let day: i64 = path.file_name()?.to_str()?.parse().ok()?;
    let month: i64 = path.parent()?.file_name()?.to_str()?.parse().ok()?;
    let year: i64 = path
        .parent()?
        .parent()?
        .file_name()?
        .to_str()?
        .parse()
        .ok()?;
    Some(days_from_civil(year, month, day))
}

fn today_path() -> String {
    date_command("+%Y/%m/%d")
}

fn today_epoch_days() -> i64 {
    let today = date_command("-u +%Y-%m-%d");
    let mut parts = today.split('-').filter_map(|p| p.parse::<i64>().ok());
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) => days_from_civil(y, m, d),
        _ => 0,
    }
}

fn now_rfc3339() -> String {
    date_command("-u +%Y-%m-%dT%H:%M:%SZ")
}

/// Days since 1970-01-01 for a Gregorian calendar date.
/// Howard Hinnant's `days_from_civil`: <http://howardhinnant.github.io/date_algorithms.html>.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Shell out to `date`; used throughout the engine to avoid a chrono
/// dependency for simple formatting (see [`crate::health::today`]). Only
/// portable `+FORMAT` invocations are used here (no GNU-only `-d`), since
/// [`days_from_civil`] handles all date arithmetic.
fn date_command(args: &str) -> String {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("date {args}"))
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn command_output(current_dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trip_via_ndjson() {
        let dir = tempfile::tempdir().unwrap();
        let entry = TelemetryRecord {
            timestamp: "2026-08-04T18:26:19Z".into(),
            git_sha: Some("abc123".into()),
            metrics: vec![MetricPoint {
                name: "ci_duration_ms".into(),
                value: 230.0,
            }],
        };
        append_record(dir.path(), &entry).unwrap();
        let file = dir
            .path()
            .join(TELEMETRY_DIR)
            .join(today_path())
            .join("history.ndjson");
        let loaded = read_ndjson(&file).unwrap();
        assert_eq!(loaded, vec![entry]);
    }

    #[test]
    fn ndjson_store_round_trips_through_the_trait() {
        let dir = tempfile::tempdir().unwrap();
        let store = NdjsonStore::new(dir.path());
        let entry = TelemetryRecord {
            timestamp: "2026-08-04T18:26:19Z".into(),
            git_sha: None,
            metrics: vec![MetricPoint {
                name: "ci_duration_ms".into(),
                value: 42.0,
            }],
        };
        store.record(entry.clone()).unwrap();
        let loaded = store.load_window(7).unwrap();
        assert_eq!(loaded, vec![entry]);
    }

    #[test]
    fn load_window_skips_dates_outside_window() {
        let dir = tempfile::tempdir().unwrap();
        let old_dir = dir.path().join(TELEMETRY_DIR).join("2020/01/01");
        std::fs::create_dir_all(&old_dir).unwrap();
        let old_entry = TelemetryRecord {
            timestamp: "2020-01-01T00:00:00Z".into(),
            git_sha: None,
            metrics: vec![MetricPoint {
                name: "x".into(),
                value: 1.0,
            }],
        };
        std::fs::write(
            old_dir.join("history.ndjson"),
            format!("{}\n", serde_json::to_string(&old_entry).unwrap()),
        )
        .unwrap();

        let records = NdjsonStore::new(dir.path()).load_window(7).unwrap();
        assert!(
            records.is_empty(),
            "record from 2020 must be outside a 7-day window"
        );
    }

    #[test]
    fn load_window_missing_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            NdjsonStore::new(dir.path())
                .load_window(7)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn latest_metric_returns_most_recent_reading() {
        let dir = tempfile::tempdir().unwrap();
        let store = NdjsonStore::new(dir.path());
        store
            .record(TelemetryRecord {
                timestamp: "2026-08-01T00:00:00Z".into(),
                git_sha: None,
                metrics: vec![MetricPoint {
                    name: "ci_duration_ms".into(),
                    value: 100.0,
                }],
            })
            .unwrap();
        store
            .record(TelemetryRecord {
                timestamp: "2026-08-02T00:00:00Z".into(),
                git_sha: None,
                metrics: vec![MetricPoint {
                    name: "ci_duration_ms".into(),
                    value: 150.0,
                }],
            })
            .unwrap();

        let latest = latest_metric(&store, "ci_duration_ms", 30).unwrap();
        assert_eq!(latest, Some(150.0));
    }

    #[test]
    fn latest_metric_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = NdjsonStore::new(dir.path());
        assert_eq!(latest_metric(&store, "ci_duration_ms", 30).unwrap(), None);
    }

    #[test]
    fn latest_metric_ignores_other_metric_names() {
        let dir = tempfile::tempdir().unwrap();
        let store = NdjsonStore::new(dir.path());
        store
            .record(TelemetryRecord {
                timestamp: "2026-08-01T00:00:00Z".into(),
                git_sha: None,
                metrics: vec![MetricPoint {
                    name: "ci_passed".into(),
                    value: 1.0,
                }],
            })
            .unwrap();
        assert_eq!(latest_metric(&store, "ci_duration_ms", 30).unwrap(), None);
    }

    #[test]
    fn record_is_noop_in_dry_run() {
        let ctx = Ctx::new(
            xshell::Shell::new().unwrap(),
            PathBuf::from("."),
            taskit_types::config::Config::default(),
            true,
            taskit_types::output_format::OutputFormat::Human,
        );
        // Would fail if it tried to write, since "." may not be writable in CI sandboxes
        // for arbitrary subdirectories; dry-run must skip the write entirely.
        assert!(record(&ctx, &[("x", 1.0)]).is_ok());
    }

    #[test]
    fn epoch_days_from_day_dir_parses_partitioned_path() {
        let path = Path::new("/root/.taskit/telemetry/2026/08/04");
        assert_eq!(
            epoch_days_from_day_dir(path),
            Some(days_from_civil(2026, 8, 4))
        );
    }

    #[test]
    fn days_from_civil_matches_known_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 8, 4), 20669);
    }
}
