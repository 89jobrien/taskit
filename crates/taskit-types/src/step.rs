use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pass,
    Fail,
    Skipped,
}

impl fmt::Display for StepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepStatus::Pass => write!(f, "PASS"),
            StepStatus::Fail => write!(f, "FAIL"),
            StepStatus::Skipped => write!(f, "SKIP"),
        }
    }
}

/// Severity level for a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

/// A single diagnostic finding from a tool (clippy warning, test failure, etc.).
#[derive(Debug, Clone)]
pub struct DiagnosticRecord {
    /// Tool-specific rule id (e.g. "clippy::needless_return", "TE001").
    pub rule_id: String,
    /// Human-readable message.
    pub message: String,
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Source file path (relative to workspace root).
    pub file: Option<String>,
    /// 1-based line number.
    pub line: Option<usize>,
    /// 1-based column number.
    pub column: Option<usize>,
}
/// A command executed while a pipeline step was running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRecord {
    /// Shell-rendered command string.
    pub command: String,
    /// Whether the command exited successfully. `None` means it was not run
    /// or the result could not be observed.
    pub success: Option<bool>,
    /// Process exit code when available.
    pub exit_code: Option<i32>,
}

/// Diagnostic context attached to a single pipeline step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepDiagnosticContext {
    /// Commands attempted while the step was running.
    pub commands: Vec<CommandRecord>,
    /// Suggested command to reproduce the step locally.
    pub reproduction: Option<String>,
    /// Free-form notes that explain additional failure context.
    pub notes: Vec<String>,
}

/// Run-level context attached to a pipeline outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PipelineRunContext {
    /// Path to the taskit binary when known.
    pub taskit_binary: Option<String>,
    /// Version of the currently running taskit binary.
    pub taskit_version: String,
    /// Workspace root used for the run.
    pub workspace_root: String,
    /// Current git commit when available.
    pub git_sha: Option<String>,
    /// `rustc --version` output when available.
    pub rustc_version: Option<String>,
    /// `cargo --version` output when available.
    pub cargo_version: Option<String>,
    /// Cargo workspace member package names.
    pub workspace_members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub gate: bool,
    /// Per-finding diagnostics captured from the tool's structured output.
    pub diagnostics: Vec<DiagnosticRecord>,
    /// Failure reproduction and command provenance for this step.
    pub context: StepDiagnosticContext,
}

#[derive(Debug, Default)]
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
    /// Best-effort run provenance for diagnostics.
    pub context: Option<PipelineRunContext>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn step_status_display() {
        assert_eq!(format!("{}", StepStatus::Pass), "PASS");
        assert_eq!(format!("{}", StepStatus::Fail), "FAIL");
        assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
    }

    #[test]
    fn pipeline_outcome_default_is_passed() {
        let outcome = PipelineOutcome {
            results: vec![],
            total: Duration::ZERO,
            passed: true,
            context: None,
        };
        assert!(outcome.passed);
    }

    #[test]
    fn diagnostic_level_debug_names_are_stable() {
        assert_eq!(format!("{:?}", DiagnosticLevel::Error), "Error");
        assert_eq!(format!("{:?}", DiagnosticLevel::Warning), "Warning");
        assert_eq!(format!("{:?}", DiagnosticLevel::Note), "Note");
    }

    #[test]
    fn diagnostic_record_fields_are_preserved() {
        let record = DiagnosticRecord {
            rule_id: "clippy::needless_return".into(),
            message: "unneeded return statement".into(),
            level: DiagnosticLevel::Warning,
            file: Some("src/lib.rs".into()),
            line: Some(10),
            column: Some(5),
        };

        assert_eq!(record.rule_id, "clippy::needless_return");
        assert_eq!(record.message, "unneeded return statement");
        assert_eq!(record.level, DiagnosticLevel::Warning);
        assert_eq!(record.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(record.line, Some(10));
        assert_eq!(record.column, Some(5));
    }

    #[test]
    fn pipeline_outcome_failed_case_preserves_failure_result() {
        let result = StepResult {
            name: "lint".into(),
            status: StepStatus::Fail,
            duration: Duration::from_millis(25),
            error: Some("clippy found errors".into()),
            gate: true,
            diagnostics: vec![DiagnosticRecord {
                rule_id: "clippy::dead_code".into(),
                message: "unused function".into(),
                level: DiagnosticLevel::Error,
                file: Some("src/lib.rs".into()),
                line: Some(42),
                column: Some(1),
            }],
            context: StepDiagnosticContext::default(),
        };
        let outcome = PipelineOutcome {
            results: vec![result],
            total: Duration::from_millis(25),
            passed: false,
            context: Some(PipelineRunContext {
                taskit_version: "0.7.0".into(),
                workspace_root: ".".into(),
                ..PipelineRunContext::default()
            }),
        };

        assert!(!outcome.passed);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].status, StepStatus::Fail);
        assert_eq!(
            outcome.results[0].error.as_deref(),
            Some("clippy found errors")
        );
        assert!(outcome.results[0].gate);
        assert_eq!(
            outcome.results[0].diagnostics[0].level,
            DiagnosticLevel::Error
        );
        assert!(outcome.context.is_some());
    }

    #[test]
    fn command_record_preserves_observed_status() {
        let record = CommandRecord {
            command: "cargo test".into(),
            success: Some(false),
            exit_code: Some(101),
        };

        assert_eq!(record.command, "cargo test");
        assert_eq!(record.success, Some(false));
        assert_eq!(record.exit_code, Some(101));
    }

    #[test]
    fn step_context_preserves_reproduction_and_commands() {
        let context = StepDiagnosticContext {
            commands: vec![CommandRecord {
                command: "cargo nextest run".into(),
                success: Some(false),
                exit_code: Some(100),
            }],
            reproduction: Some("taskit test --offline".into()),
            notes: vec!["offline tests only".into()],
        };

        assert_eq!(context.commands.len(), 1);
        assert_eq!(
            context.reproduction.as_deref(),
            Some("taskit test --offline")
        );
        assert_eq!(context.notes, vec!["offline tests only"]);
    }

    #[test]
    fn pipeline_run_context_preserves_workspace_metadata() {
        let context = PipelineRunContext {
            taskit_binary: Some("/tmp/taskit".into()),
            taskit_version: "0.7.0".into(),
            workspace_root: "/repo".into(),
            git_sha: Some("abc123".into()),
            rustc_version: Some("rustc 1.88.0".into()),
            cargo_version: Some("cargo 1.88.0".into()),
            workspace_members: vec!["taskit".into(), "taskit-engine".into()],
        };

        assert_eq!(context.taskit_binary.as_deref(), Some("/tmp/taskit"));
        assert_eq!(context.workspace_root, "/repo");
        assert_eq!(context.workspace_members, vec!["taskit", "taskit-engine"]);
    }

    // --- property tests ---

    fn arb_step_status() -> impl Strategy<Value = StepStatus> {
        prop_oneof![
            Just(StepStatus::Pass),
            Just(StepStatus::Fail),
            Just(StepStatus::Skipped),
        ]
    }

    /// Build a minimal StepResult with the given name, status, and duration (nanos).
    fn make_step(name: String, status: StepStatus, nanos: u64) -> StepResult {
        StepResult {
            name,
            status,
            duration: Duration::from_nanos(nanos),
            error: None,
            gate: false,
            diagnostics: vec![],
            context: StepDiagnosticContext::default(),
        }
    }

    proptest! {
        /// Any StepStatus Display output must be non-empty.
        #[test]
        fn prop_step_status_display_non_empty(status in arb_step_status()) {
            let s = format!("{}", status);
            prop_assert!(!s.is_empty());
        }

        /// A PipelineOutcome constructed with N all-Pass steps and passed=true
        /// preserves len(results)==N and every status is Pass.
        #[test]
        fn prop_all_pass_outcome_consistent(n in 1usize..=5) {
            let results: Vec<StepResult> = (0..n)
                .map(|i| make_step(format!("step-{i}"), StepStatus::Pass, 0))
                .collect();
            let outcome = PipelineOutcome {
                total: Duration::ZERO,
                passed: true,
                results,
                context: None,
            };
            prop_assert_eq!(outcome.results.len(), n);
            prop_assert!(outcome.results.iter().all(|r| r.status == StepStatus::Pass));
        }

        /// The outcome total duration must be >= any individual step duration.
        #[test]
        fn prop_total_gte_max_step(
            step_nanos in proptest::collection::vec(0u64..=1_000_000_000u64, 1..=5),
            extra_nanos in 0u64..=1_000_000_000u64,
        ) {
            let results: Vec<StepResult> = step_nanos
                .iter()
                .enumerate()
                .map(|(i, &nanos)| make_step(format!("step-{i}"), StepStatus::Pass, nanos))
                .collect();
            let max_step = step_nanos.iter().copied().max().unwrap_or(0);
            let total = Duration::from_nanos(max_step + extra_nanos);
            let outcome = PipelineOutcome {
                results,
                total,
                passed: true,
                context: None,
            };
            let max_individual = outcome
                .results
                .iter()
                .map(|r| r.duration)
                .max()
                .unwrap_or(Duration::ZERO);
            prop_assert!(outcome.total >= max_individual);
        }

        /// A non-empty name passed to StepResult is preserved exactly.
        #[test]
        fn prop_step_result_name_preserved(name in "[a-zA-Z0-9_\\-]{1,64}") {
            let step = make_step(name.clone(), StepStatus::Pass, 0);
            prop_assert_eq!(&step.name, &name);
        }
    }
}
