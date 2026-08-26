//! Taskit CLI entrypoint and command-line dispatch wiring.

#[allow(
    clippy::all,
    non_snake_case,
    unused_imports,
    non_camel_case_types,
    dead_code
)]
mod baml_client;
mod flow_resolver;

use clap::{CommandFactory, Parser, Subcommand};
use std::env;
use taskit_engine::changelog::ChangelogMode;
use taskit_engine::command::{
    Audit, Bench, Bootstrap, Build, Changelog, CheckDeps, CheckFreshness, CheckProtocolDrift,
    CheckProtocolSites, Ci, Clean, Command, CompileTests, Coverage, DevSetup, Drift, Flow,
    FlowAction, Fmt, Fuzz, Health, Inspect, Install, InstallHooks, Lint, Patch, PreCommit, PrePush,
    Proptest, Publish, Quick, Release, SelfCheck, SelfTest, SnapshotReview, Test, TestReport,
    TodoSync, Update, UpdateClaudeVersion, Version,
};
use taskit_engine::ctx::Ctx;
use taskit_engine::patch;
use taskit_types::config::{ConflictResolverKind, DEFAULT_COVERAGE_THRESHOLD};
use taskit_types::error::TaskitError;
use taskit_types::output_format::OutputFormat;
use xshell::Shell;

#[derive(Parser)]
#[command(name = "taskit", about = "Config-driven CI pipeline runner")]
struct Cli {
    /// Print commands without executing them
    #[arg(long, global = true)]
    dry_run: bool,
    /// Output format: human (default), json, github, junit
    #[arg(long, global = true)]
    output: Option<OutputFormat>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build, install, and workspace-setup commands
    Dev {
        #[command(subcommand)]
        sub: DevCmd,
    },
    /// Quality gates: fmt, lint, test, CI
    Check {
        #[command(subcommand)]
        sub: CheckCmd,
    },
    /// Extended testing: coverage, proptest, fuzz, bench
    Test {
        #[command(subcommand)]
        sub: TestCmd,
    },
    /// Codebase health and metrics
    Health {
        #[command(subcommand)]
        sub: HealthCmd,
    },
    /// Protocol drift, TODO sync, dependency governance
    Protocol {
        #[command(subcommand)]
        sub: ProtocolCmd,
    },
    /// Version bumps and publishing
    Release {
        #[command(subcommand)]
        sub: ReleaseCmd,
    },
    /// Git branching workflow: main -> staging -> release -> main
    Flow {
        #[command(subcommand)]
        sub: FlowCmd,
    },
    /// Operate on the taskit binary itself: install, test, check tooling
    #[command(name = "self")]
    Self_ {
        #[command(subcommand)]
        sub: SelfCmd,
    },
    /// Live terminal dashboard: workspace health, CI telemetry, and drift
    Dashboard,
    /// Generate taskit.toml and Cruxfile for the current workspace
    Init {
        /// Overwrite existing taskit.toml
        #[arg(long)]
        force: bool,
        /// Interactive mode with prompts
        #[arg(long)]
        interactive: bool,
    },
    /// Generate or preview CHANGELOG.md using git-cliff
    Changelog {
        #[command(subcommand)]
        sub: Option<ChangelogCmd>,
    },
}

#[derive(Subcommand)]
enum ChangelogCmd {
    /// Prepend unreleased changes to CHANGELOG.md
    Unreleased,
    /// Regenerate the complete CHANGELOG.md
    Full,
    /// Prepend the latest tagged release to CHANGELOG.md
    Latest,
    /// Print unreleased changes without writing a file
    Preview,
}

#[derive(Subcommand)]
enum DevCmd {
    /// Build the workspace (cargo build --workspace)
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
    },
    /// Set up a workspace for development: install git hooks and dev tools
    Bootstrap,
    /// Install git hooks that delegate to taskit
    InstallHooks,
    /// Install development tools
    Setup,
    /// Clean build artifacts
    Clean {
        #[arg(long)]
        older_than: Option<String>,
    },
    /// Update Cargo.lock dependencies
    Update {
        /// Update to latest versions, ignoring semver compatibility
        #[arg(long)]
        aggressive: bool,
    },
    /// Update pinned Claude Code version
    UpdateClaudeVersion {
        /// Version string (e.g., "2.1.50")
        version: String,
    },
}

#[derive(Subcommand)]
enum CheckCmd {
    /// Format all Rust code
    Fmt {
        /// Check only, don't modify files
        #[arg(long)]
        check: bool,
        /// Only format affected crates (git diff vs origin/main)
        #[arg(long)]
        affected: bool,
    },
    /// Run clippy lints
    Lint {
        /// Lint a specific crate
        #[arg(long, value_name = "CRATE")]
        crate_name: Option<String>,
        /// Only lint affected crates (git diff vs origin/main)
        #[arg(long)]
        affected: bool,
        /// Continue linting remaining crates even if one fails
        #[arg(long)]
        continue_on_error: bool,
        /// Auto-apply clippy's suggested fixes (--fix --allow-dirty --allow-staged)
        #[arg(long)]
        fix: bool,
    },
    /// Fast local feedback: fmt-check + lint + compile-tests + test (affected crates, offline)
    Quick,
    /// Run full local CI (all checks with summary table)
    Ci {
        /// Stop immediately after the first failed step
        #[arg(long)]
        fail_fast: bool,
        /// Include tests that require external network access or credentials (excluded by default)
        #[arg(long)]
        include_network: bool,
    },
    /// Compile all test binaries without running them
    Compile,
    /// Check for unused dependencies
    Deps,
    /// Run pre-commit checks (Rust formatting)
    PreCommit,
    /// Run pre-push checks (affected crate lint + test + coverage + drift)
    PrePush,
}

#[derive(Subcommand)]
enum SelfCmd {
    /// Install the taskit binary itself (cargo install --path . --force)
    Install,
    /// Run taskit's own test suite (hash-cached: skipped when source is unchanged)
    Test,
    /// Verify required tools are installed
    Check,
}

#[derive(Subcommand)]
enum TestCmd {
    /// Run tests via nextest
    Run {
        #[arg(long, value_name = "CRATE")]
        crate_name: Option<String>,
        #[arg(long)]
        affected: bool,
        /// Continue testing remaining crates even if one fails (implies --no-fail-fast)
        #[arg(long)]
        continue_on_error: bool,
        /// Skip tests that require external network access or credentials
        #[arg(long)]
        offline: bool,
    },
    /// Run tests with coverage
    Coverage {
        #[arg(long, value_name = "CRATE")]
        crate_name: Option<String>,
        #[arg(long, default_value_t = DEFAULT_COVERAGE_THRESHOLD)]
        threshold: f64,
        /// Measure coverage across the whole workspace instead of one crate
        #[arg(long)]
        workspace: bool,
    },
    /// Run property-based tests
    Proptest {
        /// Package to run proptests for (required)
        #[arg(long, value_name = "CRATE")]
        crate_name: String,
    },
    /// Run cargo-fuzz on a target
    Fuzz {
        /// Fuzz target name
        target: String,
        /// Duration in seconds
        #[arg(long, default_value_t = 60u64)]
        duration: u64,
    },
    /// Run criterion benchmarks
    Bench {
        #[arg(long, value_name = "CRATE")]
        crate_name: Option<String>,
        #[arg(long)]
        save_baseline: bool,
    },
    /// Generate unified coverage report
    Report,
    /// Review pending insta snapshots
    Snapshots,
}

#[derive(Subcommand)]
enum HealthCmd {
    /// Measure codebase health and compare against baseline
    Check {
        /// Write current metrics to .health-baseline.json
        #[arg(long)]
        update: bool,
        /// Also measure workspace coverage % (expensive: instrumented build)
        #[arg(long)]
        with_coverage: bool,
        /// Check only unwrap/expect and warn!() counts against the baseline,
        /// ignoring tests/clippy/coverage — for use as a CI step
        #[arg(long)]
        gate: bool,
    },
    /// Compare a telemetry metric's latest reading against its historical baseline
    Drift {
        /// Metric name (e.g. "ci_duration_ms")
        #[arg(long)]
        metric: String,
        /// Lookback window in days
        #[arg(long, default_value_t = 7)]
        window: u64,
    },
    /// Check workspace metrics against thresholds (pass/fail)
    Inspect {
        /// Maximum allowed clippy warnings (default: from config, or 0)
        #[arg(long)]
        max_warnings: Option<usize>,
        /// Maximum allowed unresolved code markers (unchecked if omitted)
        #[arg(long)]
        max_todo: Option<usize>,
    },
    /// Show workspace crate versions
    Version,
}

#[derive(Subcommand)]
enum ProtocolCmd {
    /// Check protocol drift of core contract surfaces
    Drift {
        #[arg(long)]
        update: bool,
        #[arg(long)]
        warn_only: bool,
        #[arg(long)]
        hook: bool,
        /// Continuously watch and auto-remediate drift instead of failing
        #[arg(long)]
        watch: bool,
        /// Poll interval in seconds for --watch
        #[arg(long, default_value_t = 5)]
        interval: u64,
    },
    /// Count construction sites for key structs
    Sites {
        /// File to scan
        #[arg(long)]
        file: String,
        /// Pattern to search for
        #[arg(long)]
        pattern: String,
        /// Expected count
        #[arg(long)]
        expected: usize,
        #[arg(long)]
        warn_only: bool,
    },
    /// Scan TODO/FIXME markers and sync them to GitHub issues
    TodoSync {
        #[arg(long)]
        update: bool,
        #[arg(long)]
        warn_only: bool,
    },
    /// Check workspace dependency freshness (Cargo.lock vs latest published versions)
    Freshness {
        /// Report outdated dependencies without failing the command
        #[arg(long)]
        warn_only: bool,
    },
    /// Run cargo-deny (advisories, licenses, bans)
    Audit,
}

#[derive(Subcommand)]
enum ReleaseCmd {
    /// Bump the patch version across all workspace Cargo.toml files
    Patch,
    /// Bump the minor version across all workspace Cargo.toml files
    Minor,
    /// Bump the major version across all workspace Cargo.toml files
    Major,
    /// Generate docs and publish workspace crates to crates.io
    Publish {
        /// Skip documentation generation
        #[arg(long)]
        skip_docs: bool,
        /// Allow publishing with uncommitted changes
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Create a GitHub release for a tagged version
    Create {
        /// Git tag for the release (e.g. v0.7.0)
        tag: String,
        /// Path to release notes file (uses --generate-notes if omitted)
        #[arg(long)]
        notes_file: Option<String>,
    },
}

#[derive(Subcommand)]
enum FlowCmd {
    /// Show branch positions and ahead/behind counts
    Status,
    /// Merge main into develop (bring in latest stable)
    Sync,
    /// Advance current branch one stage: develop→staging, staging→release, release→main
    Promote,
    /// Run the full pipeline across all stages with CI gate (conflict resolution requires BAML)
    Auto,
    /// Validate current branch is not protected (for pre-commit hooks)
    Guard,
}

struct NoOpResolver;
impl taskit_core::ConflictResolver for NoOpResolver {
    fn resolve(
        &self,
        _files: &[taskit_types::conflict::ConflictFile],
    ) -> Result<Vec<taskit_types::conflict::ResolvedFile>, taskit_types::error::TaskitError> {
        Err(taskit_types::error::TaskitError::other(
            "merge conflict: automatic resolution disabled (conflict_resolver = none); \
             resolve manually, then re-run `taskit flow auto`",
        ))
    }
}

struct Dashboard;
impl Command for Dashboard {
    fn run(&self, ctx: &Ctx) -> Result<(), TaskitError> {
        taskit_tui::run(ctx)
    }
}

fn make_resolver(kind: &ConflictResolverKind) -> Box<dyn taskit_core::ConflictResolver> {
    match kind {
        ConflictResolverKind::Baml => Box::new(flow_resolver::BamlConflictResolver),
        ConflictResolverKind::None => Box::new(NoOpResolver),
    }
}

/// Map a parsed CLI subcommand to its [`Command`] implementation.
///
/// This is the single dispatch seam: adding a subcommand means adding a
/// `Command` impl in `taskit-engine` and one arm here. `Init` is handled
/// before this point (it runs without a loaded config).
fn to_command(cmd: Cmd, resolver_kind: &ConflictResolverKind) -> Box<dyn Command> {
    match cmd {
        Cmd::Dev { sub } => dev_to_command(sub),
        Cmd::Check { sub } => check_to_command(sub),
        Cmd::Test { sub } => test_to_command(sub),
        Cmd::Health { sub } => health_to_command(sub),
        Cmd::Protocol { sub } => protocol_to_command(sub),
        Cmd::Release { sub } => release_to_command(sub),
        Cmd::Flow { sub } => {
            let action = match sub {
                FlowCmd::Status => FlowAction::Status,
                FlowCmd::Sync => FlowAction::Sync,
                FlowCmd::Promote => FlowAction::Promote,
                FlowCmd::Auto => FlowAction::Auto {
                    resolver: make_resolver(resolver_kind),
                    ci_runner: Box::new(|c| {
                        taskit_engine::ci::run_default_internal(c, true, false)
                    }),
                },
                FlowCmd::Guard => FlowAction::Guard,
            };
            Box::new(Flow { action })
        }
        Cmd::Self_ { sub } => self_to_command(sub),
        Cmd::Dashboard => Box::new(Dashboard),
        Cmd::Changelog { sub } => Box::new(Changelog {
            mode: match sub {
                None | Some(ChangelogCmd::Unreleased) => ChangelogMode::Unreleased,
                Some(ChangelogCmd::Full) => ChangelogMode::Full,
                Some(ChangelogCmd::Latest) => ChangelogMode::Latest,
                Some(ChangelogCmd::Preview) => ChangelogMode::Preview,
            },
        }),
        Cmd::Init { .. } => unreachable!("Init is handled before dispatch"),
    }
}

fn dev_to_command(cmd: DevCmd) -> Box<dyn Command> {
    match cmd {
        DevCmd::Build { release } => Box::new(Build { release }),
        DevCmd::Bootstrap => Box::new(Bootstrap),
        DevCmd::InstallHooks => Box::new(InstallHooks),
        DevCmd::Setup => Box::new(DevSetup),
        DevCmd::Clean { older_than } => Box::new(Clean { older_than }),
        DevCmd::Update { aggressive } => Box::new(Update { aggressive }),
        DevCmd::UpdateClaudeVersion { version } => Box::new(UpdateClaudeVersion { version }),
    }
}

fn check_to_command(cmd: CheckCmd) -> Box<dyn Command> {
    match cmd {
        CheckCmd::Fmt { check, affected } => Box::new(Fmt { check, affected }),
        CheckCmd::Lint {
            crate_name,
            affected,
            continue_on_error,
            fix,
        } => Box::new(Lint {
            crate_name,
            affected,
            continue_on_error,
            fix,
        }),
        CheckCmd::Quick => Box::new(Quick),
        CheckCmd::Ci {
            fail_fast,
            include_network,
        } => Box::new(Ci {
            fail_fast,
            include_network,
        }),
        CheckCmd::Compile => Box::new(CompileTests),
        CheckCmd::Deps => Box::new(CheckDeps),
        CheckCmd::PreCommit => Box::new(PreCommit),
        CheckCmd::PrePush => Box::new(PrePush),
    }
}

fn self_to_command(cmd: SelfCmd) -> Box<dyn Command> {
    match cmd {
        SelfCmd::Install => Box::new(Install),
        SelfCmd::Test => Box::new(SelfTest),
        SelfCmd::Check => Box::new(SelfCheck),
    }
}

fn test_to_command(cmd: TestCmd) -> Box<dyn Command> {
    match cmd {
        TestCmd::Run {
            crate_name,
            affected,
            continue_on_error,
            offline,
        } => Box::new(Test {
            crate_name,
            affected,
            continue_on_error,
            offline,
        }),
        TestCmd::Coverage {
            crate_name,
            threshold,
            workspace,
        } => Box::new(Coverage {
            crate_name,
            threshold,
            workspace,
        }),
        TestCmd::Proptest { crate_name } => Box::new(Proptest { crate_name }),
        TestCmd::Fuzz { target, duration } => Box::new(Fuzz { target, duration }),
        TestCmd::Bench {
            crate_name,
            save_baseline,
        } => Box::new(Bench {
            crate_name,
            save_baseline,
        }),
        TestCmd::Report => Box::new(TestReport),
        TestCmd::Snapshots => Box::new(SnapshotReview),
    }
}

fn health_to_command(cmd: HealthCmd) -> Box<dyn Command> {
    match cmd {
        HealthCmd::Check {
            update,
            with_coverage,
            gate,
        } => Box::new(Health {
            update,
            with_coverage,
            gate,
        }),
        HealthCmd::Drift { metric, window } => Box::new(Drift {
            metric,
            window_days: window,
        }),
        HealthCmd::Inspect {
            max_warnings,
            max_todo,
        } => Box::new(Inspect {
            max_warnings,
            max_todo,
        }),
        HealthCmd::Version => Box::new(Version),
    }
}

fn protocol_to_command(cmd: ProtocolCmd) -> Box<dyn Command> {
    match cmd {
        ProtocolCmd::Drift {
            update,
            warn_only,
            hook,
            watch,
            interval,
        } => Box::new(CheckProtocolDrift {
            update,
            warn_only,
            hook,
            watch,
            interval,
        }),
        ProtocolCmd::Sites {
            file,
            pattern,
            expected,
            warn_only,
        } => Box::new(CheckProtocolSites {
            file,
            pattern,
            expected,
            warn_only,
        }),
        ProtocolCmd::TodoSync { update, warn_only } => Box::new(TodoSync { update, warn_only }),
        ProtocolCmd::Freshness { warn_only } => Box::new(CheckFreshness { warn_only }),
        ProtocolCmd::Audit => Box::new(Audit),
    }
}

fn release_to_command(cmd: ReleaseCmd) -> Box<dyn Command> {
    match cmd {
        ReleaseCmd::Patch => Box::new(Patch {
            kind: patch::BumpKind::Patch,
        }),
        ReleaseCmd::Minor => Box::new(Patch {
            kind: patch::BumpKind::Minor,
        }),
        ReleaseCmd::Major => Box::new(Patch {
            kind: patch::BumpKind::Major,
        }),
        ReleaseCmd::Publish {
            skip_docs,
            allow_dirty,
        } => Box::new(Publish {
            skip_docs,
            allow_dirty,
        }),
        ReleaseCmd::Create { tag, notes_file } => Box::new(Release { tag, notes_file }),
    }
}

fn main() -> miette::Result<()> {
    // Not a real clap subcommand: kept out of the parsed `Cmd` tree entirely so
    // it never shows up in `clap_complete`-generated completions (clap's
    // `#[command(hide = true)]` only affects --help text, not completion
    // generators, which still walk hidden subcommands).
    if std::env::args().nth(1).as_deref() == Some("completions") {
        clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut Cli::command(),
            "taskit",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    let cli = Cli::parse();

    // Init runs before config loading (taskit.toml may not exist yet).
    if let Cmd::Init { force, interactive } = cli.cmd {
        return taskit_init::run(force, interactive, cli.dry_run).map_err(Into::into);
    }

    let workspace = taskit_engine::config::load()?;
    let workspace_root = workspace.root.clone();
    env::set_current_dir(&workspace_root)
        .map_err(taskit_types::error::TaskitError::from)
        .map_err(miette::Report::from)?;

    let sh = Shell::new()
        .map_err(taskit_types::error::TaskitError::other)
        .map_err(miette::Report::from)?;
    taskit_output::set_sink(Box::new(taskit_output::StderrSink));

    let output_format = cli.output.unwrap_or_else(|| {
        workspace
            .config
            .output
            .default_format
            .as_deref()
            .and_then(|s| clap::ValueEnum::from_str(s, true).ok())
            .unwrap_or_default()
    });

    let ctx = Ctx::new(
        sh,
        workspace_root,
        workspace.config,
        cli.dry_run,
        output_format,
    );
    let resolver_kind = ctx
        .config
        .flow
        .as_ref()
        .map(|f| f.conflict_resolver.clone())
        .unwrap_or_default();
    let command = to_command(cli.cmd, &resolver_kind);
    command.run(&ctx).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_modes_parse() {
        for args in [
            vec!["taskit", "changelog"],
            vec!["taskit", "changelog", "unreleased"],
            vec!["taskit", "changelog", "full"],
            vec!["taskit", "changelog", "latest"],
            vec!["taskit", "changelog", "preview"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
    }
}
