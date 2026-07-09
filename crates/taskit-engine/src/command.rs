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
    audit, check_deps, check_freshness, ci, clean, dev_setup, flow, fmt, health, hooks, inspect,
    lint, protocol, publish, quick, release, testing, update_claude, version,
};

/// A runnable subcommand. Implementors carry their own parsed arguments and
/// receive the shared execution context.
pub trait Command {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError>;
}

// ── formatting / linting / testing ────────────────────────────────────────

pub struct Fmt {
    pub check: bool,
    pub affected: bool,
}
impl Command for Fmt {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        fmt::run(ctx, self.check, self.affected)
    }
}

pub struct Lint {
    pub crate_name: Option<String>,
    pub affected: bool,
    pub continue_on_error: bool,
}
impl Command for Lint {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        lint::run(
            ctx,
            self.crate_name.as_deref(),
            self.affected,
            self.continue_on_error,
        )
    }
}

pub struct Test {
    pub crate_name: Option<String>,
    pub affected: bool,
    pub continue_on_error: bool,
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

pub struct Coverage {
    pub crate_name: Option<String>,
    pub threshold: f64,
}
impl Command for Coverage {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        let pkg = self
            .crate_name
            .as_deref()
            .or(ctx.cov().map(|c| c.crate_name.as_str()));
        match pkg {
            Some(name) => testing::coverage::run(ctx, name, self.threshold),
            None => Err(TaskitError::other(
                "no crate specified: use --crate-name or set [coverage].crate_name in taskit.toml",
            )),
        }
    }
}

// ── protocol ───────────────────────────────────────────────────────────────

pub struct CheckProtocolDrift {
    pub update: bool,
    pub warn_only: bool,
    pub hook: bool,
}
impl Command for CheckProtocolDrift {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        protocol::drift::run(ctx, self.update, self.warn_only, self.hook)
    }
}

pub struct CheckProtocolSites {
    pub file: String,
    pub pattern: String,
    pub expected: usize,
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

pub struct Quick;
impl Command for Quick {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        quick::run(ctx)
    }
}

pub struct Ci {
    pub fail_fast: bool,
    pub include_network: bool,
}
impl Command for Ci {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        ci::run(ctx, self.fail_fast, self.include_network)
    }
}

// ── checks / hooks / maintenance ───────────────────────────────────────────

pub struct CompileTests;
impl Command for CompileTests {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::compile::run(ctx)
    }
}

pub struct CheckDeps;
impl Command for CheckDeps {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        check_deps::run(ctx)
    }
}

pub struct CheckFreshness;
impl Command for CheckFreshness {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        check_freshness::run(ctx)
    }
}

pub struct PreCommit;
impl Command for PreCommit {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::pre_commit(ctx)
    }
}

pub struct PrePush;
impl Command for PrePush {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::pre_push(ctx)
    }
}

pub struct InstallHooks;
impl Command for InstallHooks {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        hooks::install_hooks(ctx)
    }
}

pub struct Audit;
impl Command for Audit {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        audit::run(ctx)
    }
}

pub struct Clean {
    pub older_than: Option<String>,
}
impl Command for Clean {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        clean::run(ctx, self.older_than.as_deref())
    }
}

pub struct Version;
impl Command for Version {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        version::run(ctx)
    }
}

pub struct DevSetup;
impl Command for DevSetup {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        dev_setup::setup(ctx)
    }
}

pub struct SelfCheck;
impl Command for SelfCheck {
    fn run(&self, _ctx: &Ctx) -> Result<(), TaskitError> {
        dev_setup::self_check()
    }
}

pub struct SelfTest;
impl Command for SelfTest {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::self_test::run(ctx)
    }
}

pub struct UpdateClaudeVersion {
    pub version: String,
}
impl Command for UpdateClaudeVersion {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        update_claude::run(ctx, &self.version)
    }
}

// ── extended testing ───────────────────────────────────────────────────────

pub struct Proptest {
    pub crate_name: String,
}
impl Command for Proptest {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::proptest::run(ctx, &self.crate_name)
    }
}

pub struct Fuzz {
    pub target: String,
    pub duration: u64,
}
impl Command for Fuzz {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::fuzz::run(ctx, &self.target, self.duration)
    }
}

pub struct Bench {
    pub crate_name: Option<String>,
    pub save_baseline: bool,
}
impl Command for Bench {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::bench::run(ctx, self.crate_name.as_deref(), self.save_baseline)
    }
}

pub struct TestReport;
impl Command for TestReport {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::report::run(ctx)
    }
}

pub struct SnapshotReview;
impl Command for SnapshotReview {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        testing::snapshot::run(ctx)
    }
}

// ── metrics / release ──────────────────────────────────────────────────────

pub struct Health {
    pub update: bool,
}
impl Command for Health {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        health::run(ctx, self.update)
    }
}

pub struct Inspect {
    pub max_warnings: usize,
    pub max_todo: Option<usize>,
}
impl Command for Inspect {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        inspect::run(ctx, self.max_warnings, self.max_todo)
    }
}

pub struct Publish {
    pub skip_docs: bool,
    pub allow_dirty: bool,
}
impl Command for Publish {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        publish::run(ctx, self.skip_docs, self.allow_dirty)
    }
}

pub struct Release {
    pub tag: String,
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
pub enum FlowAction {
    Status,
    Promote,
    Finish,
    Guard,
    Auto,
}

pub struct Flow {
    pub action: FlowAction,
    pub resolver: Box<dyn taskit_core::ConflictResolver>,
    pub ci_runner: Box<dyn Fn(&Ctx) -> PipelineOutcome + Send + Sync>,
}
impl Command for Flow {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        let cfg = ctx.flow();
        match &self.action {
            FlowAction::Status => flow::status(ctx, &cfg),
            FlowAction::Promote => flow::promote(ctx, &cfg),
            FlowAction::Finish => flow::finish(ctx, &cfg),
            FlowAction::Guard => flow::guard(ctx, &cfg),
            FlowAction::Auto => {
                flow::auto_with_ci(ctx, &cfg, self.resolver.as_ref(), |c| (self.ci_runner)(c))
            }
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
        };
        assert!(
            cmd.run(&ctx).is_err(),
            "coverage with no crate and no config must error"
        );
    }
}
