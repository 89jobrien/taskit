//! Command port: the extensibility seam for subcommands.
//!
//! Each subcommand is a struct holding its parsed arguments and implementing
//! [`Command`]. Dispatch is a lookup — adding a subcommand means adding a
//! struct here, not editing a central match arm in every layer. The binary
//! parses CLI flags into these structs and calls [`Command::run`] with the
//! shared [`Ctx`]. `Init` is intentionally absent: it runs before a `Ctx`
//! (and thus config) exists.

use taskit_types::error::TaskitError;

use taskit_types::step::PipelineOutcome;

use crate::ctx::Ctx;
use crate::{
    audit, bootstrap, build, changelog, check_deps, check_freshness, ci, clean, dev_setup, drift,
    flow, fmt, health, hooks, inspect, install, lint, patch, protocol, publish, quick, release,
    testing, todo_sync, update, update_claude, version,
};

/// A runnable subcommand. Implementors carry their own parsed arguments and
/// receive the shared execution context.
pub trait Command {
    /// Execute this command against the shared engine context.
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError>;
}

// ── formatting / linting / testing ────────────────────────────────────────

/// `check fmt` command options.
pub struct Fmt {
    /// Validate formatting without writing files.
    pub check: bool,
    /// Restrict formatting to affected crates.
    pub affected: bool,
}
impl Command for Fmt {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        fmt::run(ctx, self.check, self.affected)
    }
}

/// `check lint` command options.
pub struct Lint {
    /// Optional crate to lint.
    pub crate_name: Option<String>,
    /// Restrict linting to affected crates.
    pub affected: bool,
    /// Continue linting remaining crates after a failure.
    pub continue_on_error: bool,
    /// Enable clippy auto-fix mode.
    pub fix: bool,
}
impl Command for Lint {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        lint::run(
            ctx,
            self.crate_name.as_deref(),
            self.affected,
            self.continue_on_error,
            self.fix,
        )
    }
}

/// `test run` command options.
pub struct Test {
    /// Optional crate to test.
    pub crate_name: Option<String>,
    /// Restrict testing to affected crates.
    pub affected: bool,
    /// Continue testing remaining crates after a failure.
    pub continue_on_error: bool,
    /// Skip tests that require network access.
    pub offline: bool,
}
impl Command for Test {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::run::run(
            ctx,
            self.crate_name.as_deref(),
            self.affected,
            self.continue_on_error,
            self.offline,
        )
    }
}

/// `test coverage` command options.
pub struct Coverage {
    /// Optional crate to measure.
    pub crate_name: Option<String>,
    /// Minimum required coverage percentage.
    pub threshold: f64,
    /// Measure the whole workspace instead of one crate.
    pub workspace: bool,
}
impl Command for Coverage {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        if self.workspace {
            return testing::coverage::run_workspace(ctx, self.threshold);
        }
        let pkg = self
            .crate_name
            .as_deref()
            .or(ctx.cov().map(|c| c.crate_name.as_str()));
        match pkg {
            Some(name) => testing::coverage::run(ctx, name, self.threshold),
            None => Err(TaskitError::other(
                "no crate specified: use --crate-name, --workspace, or set [coverage].crate_name in taskit.toml",
            )),
        }
    }
}

// ── protocol ───────────────────────────────────────────────────────────────

/// `protocol drift` command options.
pub struct CheckProtocolDrift {
    /// Update lockfile with current hashes.
    pub update: bool,
    /// Report drift without failing.
    pub warn_only: bool,
    /// Hook mode (silent skip behavior for non-target files).
    pub hook: bool,
    /// Re-run check in watch mode.
    pub watch: bool,
    /// Watch polling interval in seconds.
    pub interval: u64,
}
impl Command for CheckProtocolDrift {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        if self.watch {
            return protocol::drift::watch(ctx, self.interval);
        }
        protocol::drift::run(ctx, self.update, self.warn_only, self.hook)
    }
}

/// `protocol todo-sync` command options.
pub struct TodoSync {
    /// Persist sync lock updates.
    pub update: bool,
    /// Report unsynced markers without failing.
    pub warn_only: bool,
}
impl Command for TodoSync {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        todo_sync::run(ctx, self.update, self.warn_only)
    }
}

/// `protocol sites` command options.
pub struct CheckProtocolSites {
    /// File path to scan.
    pub file: String,
    /// Substring pattern to count.
    pub pattern: String,
    /// Expected number of matches.
    pub expected: usize,
    /// Report mismatch without failing.
    pub warn_only: bool,
}
impl Command for CheckProtocolSites {
    fn run(&self, _ctx: &Ctx) -> Result<(), TaskitError> {
        protocol::sites::run(
            std::path::Path::new(&self.file),
            &self.pattern,
            self.expected,
            self.warn_only,
        )
    }
}

// ── pipelines ────────────────────────────────────────────────────────────

/// `check quick` command.
pub struct Quick;
impl Command for Quick {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        quick::run(ctx)
    }
}

/// `check ci` command options.
pub struct Ci {
    /// Stop on first failing step.
    pub fail_fast: bool,
    /// Include tests marked as network-dependent.
    pub include_network: bool,
}
impl Command for Ci {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        ci::run(ctx, self.fail_fast, self.include_network)
    }
}

// ── checks / hooks / maintenance ───────────────────────────────────────────

/// `check compile` command.
pub struct CompileTests;
impl Command for CompileTests {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::compile::run(ctx)
    }
}

/// `check deps` command.
pub struct CheckDeps;
impl Command for CheckDeps {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        check_deps::run(ctx)
    }
}

/// `changelog` command options.
pub struct Changelog {
    /// Changelog workflow to run.
    pub mode: changelog::ChangelogMode,
}
impl Command for Changelog {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        changelog::run(ctx, self.mode)
    }
}

/// `protocol freshness` command options.
pub struct CheckFreshness {
    /// Report stale dependencies without failing.
    pub warn_only: bool,
}
impl Command for CheckFreshness {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        check_freshness::run(ctx, self.warn_only)
    }
}

/// `check pre-commit` command.
pub struct PreCommit;
impl Command for PreCommit {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::pre_commit(ctx)
    }
}

/// `check pre-push` command.
pub struct PrePush;
impl Command for PrePush {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::pre_push(ctx)
    }
}

/// `dev install-hooks` command.
pub struct InstallHooks;
impl Command for InstallHooks {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::install_hooks(ctx)
    }
}

/// `dev bootstrap` command.
pub struct Bootstrap;
impl Command for Bootstrap {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        bootstrap::run(ctx)
    }
}

/// `dev build` command options.
pub struct Build {
    /// Build in release mode.
    pub release: bool,
}
impl Command for Build {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        build::run(ctx, self.release)
    }
}

/// `dev install` command.
pub struct Install;
impl Command for Install {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        install::run(ctx)
    }
}

/// `dev update` command options.
pub struct Update {
    /// Allow aggressive dependency updates.
    pub aggressive: bool,
}
impl Command for Update {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        update::run(ctx, self.aggressive)
    }
}

/// `release patch|minor|major` command.
pub struct Patch {
    /// Semantic bump kind to apply.
    pub kind: patch::BumpKind,
}
impl Command for Patch {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        patch::run(ctx, self.kind)
    }
}

/// `protocol audit` command.
pub struct Audit;
impl Command for Audit {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        audit::run(ctx)
    }
}

/// `dev clean` command options.
pub struct Clean {
    /// Optional age selector for `cargo sweep` style cleanup.
    pub older_than: Option<String>,
}
impl Command for Clean {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        clean::run(ctx, self.older_than.as_deref())
    }
}

/// `health version` command.
pub struct Version;
impl Command for Version {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        version::run(ctx)
    }
}

/// `dev setup` command.
pub struct DevSetup;
impl Command for DevSetup {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        dev_setup::setup(ctx)
    }
}

/// `self check` command.
pub struct SelfCheck;
impl Command for SelfCheck {
    fn run(&self, _ctx: &Ctx) -> Result<(), TaskitError> {
        dev_setup::self_check()
    }
}

/// `self test` command.
pub struct SelfTest;
impl Command for SelfTest {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::self_test::run(ctx)
    }
}

/// `dev update-claude-version` command options.
pub struct UpdateClaudeVersion {
    /// Target Claude Code version string.
    pub version: String,
}
impl Command for UpdateClaudeVersion {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        update_claude::run(ctx, &self.version)
    }
}

// ── extended testing ───────────────────────────────────────────────────────

/// `test proptest` command options.
pub struct Proptest {
    /// Crate/package to run proptests for.
    pub crate_name: String,
}
impl Command for Proptest {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::proptest::run(ctx, &self.crate_name)
    }
}

/// `test fuzz` command options.
pub struct Fuzz {
    /// Fuzz target name.
    pub target: String,
    /// Run duration in seconds.
    pub duration: u64,
}
impl Command for Fuzz {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::fuzz::run(ctx, &self.target, self.duration)
    }
}

/// `test bench` command options.
pub struct Bench {
    /// Optional crate/package to benchmark.
    pub crate_name: Option<String>,
    /// Save criterion baseline after run.
    pub save_baseline: bool,
}
impl Command for Bench {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::bench::run(ctx, self.crate_name.as_deref(), self.save_baseline)
    }
}

/// `test report` command.
pub struct TestReport;
impl Command for TestReport {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::report::run(ctx)
    }
}

/// `test snapshots` command.
pub struct SnapshotReview;
impl Command for SnapshotReview {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::snapshot::run(ctx)
    }
}

// ── metrics / release ──────────────────────────────────────────────────────

/// `health check` command options.
pub struct Health {
    /// Write a new health baseline.
    pub update: bool,
    /// Collect workspace coverage during baseline collection.
    pub with_coverage: bool,
    /// Run only the safety gate.
    pub gate: bool,
}
impl Command for Health {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        health::run(ctx, self.update, self.with_coverage, self.gate)
    }
}

/// `health drift` command options.
pub struct Drift {
    /// Metric name to analyze.
    pub metric: String,
    /// Lookback window in days.
    pub window_days: u64,
}
impl Command for Drift {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        drift::run(ctx, &self.metric, self.window_days)
    }
}

/// `health inspect` command options.
pub struct Inspect {
    /// Optional override for max clippy warnings.
    pub max_warnings: Option<usize>,
    /// Optional override for max TODO/FIXME markers.
    pub max_todo: Option<usize>,
}
impl Command for Inspect {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        inspect::run(ctx, self.max_warnings, self.max_todo)
    }
}

/// `release publish` command options.
pub struct Publish {
    /// Skip `cargo doc` before publishing.
    pub skip_docs: bool,
    /// Allow publishing from a dirty worktree.
    pub allow_dirty: bool,
}
impl Command for Publish {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        publish::run(ctx, self.skip_docs, self.allow_dirty)
    }
}

/// `release create` command options.
pub struct Release {
    /// Release tag to publish.
    pub tag: String,
    /// Optional release notes file.
    pub notes_file: Option<String>,
}
impl Command for Release {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        let notes = self.notes_file.as_ref().map(std::path::Path::new);
        release::gh::run(ctx, &self.tag, notes)
    }
}

// ── flow ───────────────────────────────────────────────────────────────────

#[non_exhaustive]
/// Flow sub-actions for the `flow` command.
pub enum FlowAction {
    /// Print flow branch status.
    Status,
    /// Sync main into develop.
    Sync,
    /// Promote one stage forward.
    Promote,
    /// Enforce protected-branch guardrails.
    Guard,
    /// Run full automated promote/CI/finish flow.
    Auto {
        /// Conflict resolver implementation.
        resolver: Box<dyn taskit_core::ConflictResolver>,
        /// CI runner closure used in flow auto.
        ci_runner: Box<dyn Fn(&Ctx) -> PipelineOutcome + Send + Sync>,
    },
}

/// `flow` command options.
pub struct Flow {
    /// Selected flow action.
    pub action: FlowAction,
}
impl Command for Flow {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        let cfg = ctx.flow();
        match &self.action {
            FlowAction::Status => flow::status(ctx, &cfg),
            FlowAction::Sync => flow::sync(ctx, &cfg),
            FlowAction::Promote => flow::promote(ctx, &cfg),
            FlowAction::Guard => flow::guard(ctx, &cfg),
            FlowAction::Auto {
                resolver,
                ci_runner,
            } => flow::auto_with_ci(ctx, &cfg, resolver.as_ref(), |c| ci_runner(c)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_check_command_dispatches() {
        // SelfCheck ignores ctx and verifies tool availability; it must be
        // invokable through the trait object without panicking.
        let ctx = Ctx::test();
        let cmd: Box<dyn Command> = Box::new(SelfCheck);
        // Result depends on host tooling; we only assert it runs to a Result.
        let _ = cmd.run(&ctx);
    }

    #[test]
    fn coverage_without_crate_is_err() {
        let ctx = Ctx::test();
        let cmd = Coverage {
            crate_name: None,
            threshold: 80.0,
            workspace: false,
        };
        assert!(
            cmd.run(&ctx).is_err(),
            "coverage with no crate and no config must error"
        );
    }
}
