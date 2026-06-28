use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;
use taskit_types::error::TaskitError;
use xshell::{Shell, cmd};

const BASELINE_FILE: &str = ".health-baseline.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthBaseline {
    pub date: String,
    pub tests: TestCounts,
    pub clippy: ClippyCounts,
    pub todo_fixme: usize,
    pub crates: usize,
    pub versions_consistent: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClippyCounts {
    pub warnings: usize,
    pub errors: usize,
}

/// Collect a fresh health baseline from the workspace.
pub fn collect(sh: &Shell) -> Result<HealthBaseline, TaskitError> {
    let tests = collect_tests(sh)?;
    let clippy = collect_clippy(sh)?;
    let todo_fixme = count_todo_fixme(sh)?;
    let (crate_count, versions_consistent, version) = collect_versions()?;

    let date = today();

    Ok(HealthBaseline {
        date,
        tests,
        clippy,
        todo_fixme,
        crates: crate_count,
        versions_consistent,
        version,
    })
}

/// Load an existing baseline from `.health-baseline.json`.
pub fn load_baseline(workspace_root: &Path) -> Result<HealthBaseline, TaskitError> {
    let path = workspace_root.join(BASELINE_FILE);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("no baseline found at {}", path.display()))?;
    Ok(serde_json::from_str(&content).context("failed to parse health baseline")?)
}

/// Write a baseline to `.health-baseline.json`.
pub fn write_baseline(workspace_root: &Path, baseline: &HealthBaseline) -> Result<(), TaskitError> {
    let path = workspace_root.join(BASELINE_FILE);
    let json = serde_json::to_string_pretty(baseline)
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;
    Ok(std::fs::write(&path, format!("{json}\n")).context("failed to write health baseline")?)
}

/// Compare current against a previous baseline and print a report.
/// Returns `Ok(true)` if no regressions, `Ok(false)` if regressions found.
pub fn check(current: &HealthBaseline, previous: &HealthBaseline) -> bool {
    let mut regressed = false;

    eprintln!("Health Check (baseline: {})", previous.date);
    eprintln!("{}", "-".repeat(50));

    regressed |= print_metric(
        "Tests (total)",
        previous.tests.total,
        current.tests.total,
        Direction::HigherIsBetter,
    );
    regressed |= print_metric(
        "Tests (failed)",
        previous.tests.failed,
        current.tests.failed,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "Clippy warnings",
        previous.clippy.warnings,
        current.clippy.warnings,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "Clippy errors",
        previous.clippy.errors,
        current.clippy.errors,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "TODO/FIXME",
        previous.todo_fixme,
        current.todo_fixme,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "Crates",
        previous.crates,
        current.crates,
        Direction::Neutral,
    );

    if !current.versions_consistent {
        eprintln!(
            "  Versions consistent:  no (was: {})",
            previous.versions_consistent
        );
        regressed = true;
    } else {
        eprintln!("  Versions consistent:  yes");
    }

    eprintln!("{}", "-".repeat(50));
    if regressed {
        eprintln!("REGRESSION detected");
    } else {
        eprintln!("No regressions");
    }

    !regressed
}

/// Run the health subcommand.
pub fn run(sh: &Shell, workspace_root: &Path, update: bool) -> Result<(), TaskitError> {
    let current = collect(sh)?;

    if update {
        write_baseline(workspace_root, &current)?;
        eprintln!("Baseline written to {BASELINE_FILE}");
        print_summary(&current);
        return Ok(());
    }

    match load_baseline(workspace_root) {
        Ok(previous) => {
            if check(&current, &previous) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("health regression detected").into())
            }
        }
        Err(_) => {
            eprintln!("No existing baseline found. Current health:");
            print_summary(&current);
            eprintln!("\nRun `taskit health --update` to create a baseline.");
            Ok(())
        }
    }
}

fn print_summary(b: &HealthBaseline) {
    eprintln!(
        "  Tests:       {} total, {} passed, {} failed, {} skipped",
        b.tests.total, b.tests.passed, b.tests.failed, b.tests.skipped
    );
    eprintln!(
        "  Clippy:      {} warnings, {} errors",
        b.clippy.warnings, b.clippy.errors
    );
    eprintln!("  TODO/FIXME:  {}", b.todo_fixme);
    eprintln!("  Crates:      {}", b.crates);
    eprintln!(
        "  Version:     {} (consistent: {})",
        b.version, b.versions_consistent
    );
}

// -- Collectors ---------------------------------------------------------------

fn collect_tests(sh: &Shell) -> Result<TestCounts, TaskitError> {
    // Run nextest and parse its output. Nextest exits non-zero on failures,
    // so we capture the output regardless of exit code.
    let output = cmd!(sh, "cargo nextest run --workspace --no-fail-fast")
        .ignore_status()
        .read_stderr()
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;
    parse_nextest_summary(&output)
}

fn collect_clippy(sh: &Shell) -> Result<ClippyCounts, TaskitError> {
    let output = cmd!(sh, "cargo clippy --workspace --message-format=json")
        .ignore_status()
        .read()
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;
    Ok(parse_clippy_json(&output))
}

fn count_todo_fixme(sh: &Shell) -> Result<usize, TaskitError> {
    // Search .rs files under crates/ and src/ for TODO or FIXME
    let output = cmd!(sh, "grep -r -c -E TODO|FIXME --include=*.rs crates/ src/")
        .ignore_status()
        .read()
        .map_err(|e| TaskitError::from(anyhow::anyhow!("{e}")))?;
    Ok(parse_grep_counts(&output))
}

fn collect_versions() -> Result<(usize, bool, String), TaskitError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context("cargo metadata failed")?;

    let packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .collect();

    let crate_count = packages.len();
    let versions: Vec<String> = packages.iter().map(|p| p.version.to_string()).collect();

    let version = versions.first().cloned().unwrap_or_default();
    let consistent = versions.iter().all(|v| *v == version);

    Ok((crate_count, consistent, version))
}

fn today() -> String {
    // Use a simple date without external deps
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    output.trim().to_string()
}

// -- Parsers (pure, testable) -------------------------------------------------

/// Parse nextest's summary line like:
/// `341 tests run: 341 passed, 0 skipped`
/// or on failure: `341 tests run: 338 passed, 3 failed, 0 skipped`
fn parse_nextest_summary(output: &str) -> Result<TestCounts, TaskitError> {
    // Look for the summary line from nextest
    for line in output.lines().rev() {
        let line = line.trim();
        // nextest prints: "N tests run: N passed, N failed, N skipped"
        // or:             "N tests run: N passed, N skipped"
        if let Some(rest) = line.strip_suffix(" run.") {
            // Alternative format
            if let Some(counts) = try_parse_summary_line(rest) {
                return Ok(counts);
            }
        }
        if line.contains("tests run:")
            && let Some(counts) = try_parse_summary_line(line)
        {
            return Ok(counts);
        }
    }
    // Fallback: count individual test lines
    let passed = output.matches("PASS [").count();
    let failed = output.matches("FAIL [").count();
    let skipped = output.matches("SKIP [").count();
    let total = passed + failed + skipped;
    Ok(TestCounts {
        total,
        passed,
        failed,
        skipped,
    })
}

fn try_parse_summary_line(line: &str) -> Option<TestCounts> {
    // Extract numbers from patterns like "341 tests run: 341 passed, 3 failed, 0 skipped"
    let nums: Vec<usize> = line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();

    match nums.len() {
        // total, passed, skipped (no failures)
        3 => Some(TestCounts {
            total: nums[0],
            passed: nums[1],
            failed: 0,
            skipped: nums[2],
        }),
        // total, passed, failed, skipped
        4 => Some(TestCounts {
            total: nums[0],
            passed: nums[1],
            failed: nums[2],
            skipped: nums[3],
        }),
        _ => None,
    }
}

fn parse_clippy_json(output: &str) -> ClippyCounts {
    let mut warnings = 0;
    let mut errors = 0;
    for line in output.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(level) = msg.pointer("/message/level").and_then(|l| l.as_str()) else {
            continue;
        };
        match level {
            "warning" => {
                // Skip "N warnings generated" summary messages
                let code = msg.pointer("/message/code/code").and_then(|c| c.as_str());
                if code.is_some() {
                    warnings += 1;
                }
            }
            "error" => errors += 1,
            _ => {}
        }
    }
    ClippyCounts { warnings, errors }
}

fn parse_grep_counts(output: &str) -> usize {
    output
        .lines()
        .filter_map(|line| {
            // grep -c output: "path:N"
            line.rsplit_once(':')
                .and_then(|(_, n)| n.trim().parse::<usize>().ok())
        })
        .sum()
}

// -- Direction for metric comparison ------------------------------------------

#[derive(Clone, Copy)]
enum Direction {
    HigherIsBetter,
    LowerIsBetter,
    Neutral,
}

/// Print a metric comparison line. Returns `true` if regressed.
fn print_metric(name: &str, previous: usize, current: usize, direction: Direction) -> bool {
    let arrow = match current.cmp(&previous) {
        std::cmp::Ordering::Greater => "^",
        std::cmp::Ordering::Less => "v",
        std::cmp::Ordering::Equal => "=",
    };
    let regressed = match direction {
        Direction::HigherIsBetter => current < previous,
        Direction::LowerIsBetter => current > previous,
        Direction::Neutral => false,
    };
    let marker = if regressed { " REGRESSION" } else { "" };
    eprintln!("  {name:<20} {previous:>5} -> {current:>5} {arrow}{marker}");
    regressed
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_nextest_summary --

    #[test]
    fn parse_nextest_all_pass() {
        let output = "some lines\n   341 tests run: 341 passed, 0 skipped\nmore lines";
        let counts = parse_nextest_summary(output).unwrap();
        assert_eq!(counts.total, 341);
        assert_eq!(counts.passed, 341);
        assert_eq!(counts.failed, 0);
        assert_eq!(counts.skipped, 0);
    }

    #[test]
    fn parse_nextest_with_failures() {
        let output = "   100 tests run: 97 passed, 2 failed, 1 skipped";
        let counts = parse_nextest_summary(output).unwrap();
        assert_eq!(counts.total, 100);
        assert_eq!(counts.passed, 97);
        assert_eq!(counts.failed, 2);
        assert_eq!(counts.skipped, 1);
    }

    #[test]
    fn parse_nextest_fallback_to_line_counting() {
        let output = "PASS [  0.1s] crate::test_a\nPASS [  0.2s] crate::test_b\nFAIL [  0.3s] crate::test_c\n";
        let counts = parse_nextest_summary(output).unwrap();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.passed, 2);
        assert_eq!(counts.failed, 1);
    }

    // -- parse_clippy_json --

    #[test]
    fn parse_clippy_json_counts_warnings() {
        let line = r#"{"reason":"compiler-message","message":{"level":"warning","code":{"code":"dead_code"},"message":"unused"}}"#;
        let counts = parse_clippy_json(line);
        assert_eq!(counts.warnings, 1);
        assert_eq!(counts.errors, 0);
    }

    #[test]
    fn parse_clippy_json_skips_summary_warnings() {
        // Summary warnings have no code
        let line = r#"{"reason":"compiler-message","message":{"level":"warning","message":"5 warnings generated"}}"#;
        let counts = parse_clippy_json(line);
        assert_eq!(counts.warnings, 0);
    }

    #[test]
    fn parse_clippy_json_counts_errors() {
        let line = r#"{"reason":"compiler-message","message":{"level":"error","code":{"code":"E0433"},"message":"not found"}}"#;
        let counts = parse_clippy_json(line);
        assert_eq!(counts.errors, 1);
    }

    #[test]
    fn parse_clippy_json_empty() {
        assert_eq!(parse_clippy_json("").warnings, 0);
        assert_eq!(parse_clippy_json("").errors, 0);
    }

    #[test]
    fn parse_clippy_json_ignores_non_message_lines() {
        let lines = r#"{"reason":"build-script-executed"}
{"reason":"compiler-artifact","target":{"name":"foo"}}"#;
        let counts = parse_clippy_json(lines);
        assert_eq!(counts.warnings, 0);
        assert_eq!(counts.errors, 0);
    }

    // -- parse_grep_counts --

    #[test]
    fn parse_grep_counts_sums_lines() {
        let output = "src/main.rs:2\ncrates/foo/src/lib.rs:3\ncrates/bar/src/lib.rs:0\n";
        assert_eq!(parse_grep_counts(output), 5);
    }

    #[test]
    fn parse_grep_counts_empty() {
        assert_eq!(parse_grep_counts(""), 0);
    }

    // -- check --

    #[test]
    fn check_no_regression() {
        let prev = HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        let current = HealthBaseline {
            date: "2026-01-02".into(),
            tests: TestCounts {
                total: 110,
                passed: 110,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 4,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        assert!(check(&current, &prev));
    }

    #[test]
    fn check_test_count_regression() {
        let prev = HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        let current = HealthBaseline {
            date: "2026-01-02".into(),
            tests: TestCounts {
                total: 90,
                passed: 90,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        assert!(!check(&current, &prev));
    }

    #[test]
    fn check_clippy_regression() {
        let prev = HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        let current = HealthBaseline {
            date: "2026-01-02".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 2,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        assert!(!check(&current, &prev));
    }

    #[test]
    fn check_version_inconsistency_is_regression() {
        let prev = HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        let current = HealthBaseline {
            date: "2026-01-02".into(),
            tests: TestCounts {
                total: 100,
                passed: 100,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 5,
            crates: 3,
            versions_consistent: false,
            version: "0.1.0".into(),
        };
        assert!(!check(&current, &prev));
    }

    // -- serialization round-trip --

    #[test]
    fn baseline_round_trip() {
        let baseline = HealthBaseline {
            date: "2026-06-28".into(),
            tests: TestCounts {
                total: 341,
                passed: 341,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 8,
            crates: 5,
            versions_consistent: true,
            version: "0.4.0".into(),
        };
        let json = serde_json::to_string_pretty(&baseline).unwrap();
        let parsed: HealthBaseline = serde_json::from_str(&json).unwrap();
        assert_eq!(baseline, parsed);
    }

    // -- write/load round-trip --

    #[test]
    fn write_and_load_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let baseline = HealthBaseline {
            date: "2026-06-28".into(),
            tests: TestCounts {
                total: 10,
                passed: 10,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 2,
            crates: 1,
            versions_consistent: true,
            version: "0.1.0".into(),
        };
        write_baseline(dir.path(), &baseline).unwrap();
        let loaded = load_baseline(dir.path()).unwrap();
        assert_eq!(baseline, loaded);
    }

    #[test]
    fn load_baseline_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_baseline(dir.path()).is_err());
    }

    // -- print_metric --

    #[test]
    fn metric_higher_is_better_regression() {
        assert!(print_metric("test", 100, 90, Direction::HigherIsBetter));
    }

    #[test]
    fn metric_higher_is_better_improvement() {
        assert!(!print_metric("test", 100, 110, Direction::HigherIsBetter));
    }

    #[test]
    fn metric_lower_is_better_regression() {
        assert!(print_metric("test", 0, 5, Direction::LowerIsBetter));
    }

    #[test]
    fn metric_lower_is_better_improvement() {
        assert!(!print_metric("test", 5, 0, Direction::LowerIsBetter));
    }

    #[test]
    fn metric_neutral_never_regresses() {
        assert!(!print_metric("test", 3, 5, Direction::Neutral));
        assert!(!print_metric("test", 5, 3, Direction::Neutral));
    }
}
