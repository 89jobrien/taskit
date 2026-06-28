use anyhow::Result;
use xshell::Shell;

use crate::health::{self, HealthBaseline};

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub max_clippy_warnings: usize,
    pub max_clippy_errors: usize,
    pub max_test_failures: usize,
    pub max_todo_fixme: Option<usize>,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            max_clippy_warnings: 0,
            max_clippy_errors: 0,
            max_test_failures: 0,
            max_todo_fixme: None,
        }
    }
}

struct Check {
    name: &'static str,
    value: usize,
    limit: usize,
    passed: bool,
}

fn evaluate(baseline: &HealthBaseline, thresholds: &Thresholds) -> Vec<Check> {
    let mut checks = vec![
        {
            let v = baseline.tests.failed;
            let l = thresholds.max_test_failures;
            Check {
                name: "Test failures",
                value: v,
                limit: l,
                passed: v <= l,
            }
        },
        {
            let v = baseline.clippy.errors;
            let l = thresholds.max_clippy_errors;
            Check {
                name: "Clippy errors",
                value: v,
                limit: l,
                passed: v <= l,
            }
        },
        {
            let v = baseline.clippy.warnings;
            let l = thresholds.max_clippy_warnings;
            Check {
                name: "Clippy warnings",
                value: v,
                limit: l,
                passed: v <= l,
            }
        },
        Check {
            name: "Version consistency",
            value: if baseline.versions_consistent { 0 } else { 1 },
            limit: 0,
            passed: baseline.versions_consistent,
        },
    ];

    if let Some(max_todo) = thresholds.max_todo_fixme {
        checks.push({
            let v = baseline.todo_fixme;
            Check {
                name: "TODO/FIXME count",
                value: v,
                limit: max_todo,
                passed: v <= max_todo,
            }
        });
    }

    checks
}

fn print_report(baseline: &HealthBaseline, checks: &[Check]) -> bool {
    eprintln!("taskit inspect");
    eprintln!("{}", "-".repeat(55));
    eprintln!(
        "  Tests:  {} total, {} passed, {} failed, {} skipped",
        baseline.tests.total, baseline.tests.passed, baseline.tests.failed, baseline.tests.skipped,
    );
    eprintln!(
        "  Clippy: {} warnings, {} errors",
        baseline.clippy.warnings, baseline.clippy.errors,
    );
    eprintln!("  TODO/FIXME: {}", baseline.todo_fixme);
    eprintln!(
        "  Crates: {} (version: {}, consistent: {})",
        baseline.crates, baseline.version, baseline.versions_consistent,
    );
    eprintln!("{}", "-".repeat(55));

    let mut all_passed = true;
    for c in checks {
        let status = if c.passed { "PASS" } else { "FAIL" };
        if !c.passed {
            all_passed = false;
        }
        eprintln!(
            "  [{status}] {:<22} {} (limit: {})",
            c.name, c.value, c.limit,
        );
    }

    eprintln!("{}", "-".repeat(55));
    if all_passed {
        eprintln!("Result: PASS");
    } else {
        eprintln!("Result: FAIL");
    }
    all_passed
}

pub fn run(sh: &Shell, max_warnings: usize, max_todo: Option<usize>) -> Result<()> {
    let baseline = health::collect(sh)?;
    let thresholds = Thresholds {
        max_clippy_warnings: max_warnings,
        max_todo_fixme: max_todo,
        ..Default::default()
    };
    let checks = evaluate(&baseline, &thresholds);
    if print_report(&baseline, &checks) {
        Ok(())
    } else {
        anyhow::bail!("inspect failed: one or more checks did not pass")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ClippyCounts, HealthBaseline, TestCounts};

    fn baseline(
        failed: usize,
        warnings: usize,
        errors: usize,
        todo: usize,
        consistent: bool,
    ) -> HealthBaseline {
        HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100 - failed,
                failed,
                skipped: 0,
            },
            clippy: ClippyCounts { warnings, errors },
            todo_fixme: todo,
            crates: 4,
            versions_consistent: consistent,
            version: "0.4.0".into(),
        }
    }

    #[test]
    fn all_clean_passes() {
        let b = baseline(0, 0, 0, 3, true);
        let checks = evaluate(&b, &Thresholds::default());
        assert!(checks.iter().all(|c| c.passed));
    }

    #[test]
    fn test_failures_fail() {
        let b = baseline(1, 0, 0, 0, true);
        let checks = evaluate(&b, &Thresholds::default());
        let fail_check = checks.iter().find(|c| c.name == "Test failures").unwrap();
        assert!(!fail_check.passed);
    }

    #[test]
    fn clippy_errors_fail() {
        let b = baseline(0, 0, 1, 0, true);
        let checks = evaluate(&b, &Thresholds::default());
        let check = checks.iter().find(|c| c.name == "Clippy errors").unwrap();
        assert!(!check.passed);
    }

    #[test]
    fn clippy_warnings_within_threshold_pass() {
        let b = baseline(0, 3, 0, 0, true);
        let t = Thresholds {
            max_clippy_warnings: 5,
            ..Default::default()
        };
        let checks = evaluate(&b, &t);
        let check = checks.iter().find(|c| c.name == "Clippy warnings").unwrap();
        assert!(check.passed);
    }

    #[test]
    fn clippy_warnings_over_threshold_fail() {
        let b = baseline(0, 6, 0, 0, true);
        let t = Thresholds {
            max_clippy_warnings: 5,
            ..Default::default()
        };
        let checks = evaluate(&b, &t);
        let check = checks.iter().find(|c| c.name == "Clippy warnings").unwrap();
        assert!(!check.passed);
    }

    #[test]
    fn version_inconsistency_fails() {
        let b = baseline(0, 0, 0, 0, false);
        let checks = evaluate(&b, &Thresholds::default());
        let check = checks
            .iter()
            .find(|c| c.name == "Version consistency")
            .unwrap();
        assert!(!check.passed);
    }

    #[test]
    fn todo_not_checked_without_threshold() {
        let b = baseline(0, 0, 0, 100, true);
        let checks = evaluate(&b, &Thresholds::default());
        assert!(checks.iter().all(|c| c.passed));
        assert!(!checks.iter().any(|c| c.name == "TODO/FIXME count"));
    }

    #[test]
    fn todo_checked_when_threshold_set() {
        let b = baseline(0, 0, 0, 10, true);
        let t = Thresholds {
            max_todo_fixme: Some(5),
            ..Default::default()
        };
        let checks = evaluate(&b, &t);
        let check = checks
            .iter()
            .find(|c| c.name == "TODO/FIXME count")
            .unwrap();
        assert!(!check.passed);
    }

    #[test]
    fn print_report_returns_true_on_all_pass() {
        let b = baseline(0, 0, 0, 0, true);
        let checks = evaluate(&b, &Thresholds::default());
        assert!(print_report(&b, &checks));
    }

    #[test]
    fn print_report_returns_false_on_failure() {
        let b = baseline(1, 0, 0, 0, true);
        let checks = evaluate(&b, &Thresholds::default());
        assert!(!print_report(&b, &checks));
    }
}
