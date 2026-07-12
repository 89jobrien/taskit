#[allow(
    clippy::all,
    non_snake_case,
    unused_imports,
    non_camel_case_types,
    dead_code
)]
mod baml_client;
mod flow_resolver;

use clap::{Parser, Subcommand};
use std::env;
use taskit_engine::command::{self, Command};
use taskit_engine::ctx::Ctx;
use taskit_engine::patch;
use taskit_types::config::{ConflictResolverKind, DEFAULT_COVERAGE_THRESHOLD};
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
    },
    /// Run tests via nextest
    Test {
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
    },
    /// Check protocol drift of core contract surfaces
    CheckProtocolDrift {
        #[arg(long)]
        update: bool,
        #[arg(long)]
        warn_only: bool,
        #[arg(long)]
        hook: bool,
    },
    /// Count construction sites for key structs
    CheckProtocolSites {
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
    CompileTests,
    /// Check for unused dependencies
    CheckDeps,
    /// Check schema + protocol drift freshness
    CheckFreshness,
    /// Run pre-commit checks (Rust formatting)
    PreCommit,
    /// Run pre-push checks (affected crate lint + test + coverage + drift)
    PrePush,
    /// Install git hooks that delegate to taskit
    InstallHooks,
    /// Install hooks and dev tools (workspace bootstrap)
    Install,
    /// Update Cargo.lock dependencies
    Update {
        /// Update to latest versions, ignoring semver compatibility
        #[arg(long)]
        aggressive: bool,
    },
    /// Bump the patch version across all workspace Cargo.toml files
    Patch,
    /// Bump the minor version across all workspace Cargo.toml files
    Minor,
    /// Bump the major version across all workspace Cargo.toml files
    Major,
    /// Run cargo-deny (advisories, licenses, bans)
    Audit,
    /// Clean build artifacts
    Clean {
        #[arg(long)]
        older_than: Option<String>,
    },
    /// Show workspace crate versions
    Version,
    /// Install development tools
    DevSetup,
    /// Verify required tools are installed
    SelfCheck,
    /// Run taskit's own test suite (hash-cached: skipped when source is unchanged)
    SelfTest,
    /// Update pinned Claude Code version
    UpdateClaudeVersion {
        /// Version string (e.g., "2.1.50")
        version: String,
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
    TestReport,
    /// Review pending insta snapshots
    SnapshotReview,
    /// Measure codebase health and compare against baseline
    Health {
        /// Write current metrics to .health-baseline.json
        #[arg(long)]
        update: bool,
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
    Release {
        /// Git tag for the release (e.g. v0.7.0)
        tag: String,
        /// Path to release notes file (uses --generate-notes if omitted)
        #[arg(long)]
        notes_file: Option<String>,
    },
    /// Git branching workflow: main -> staging -> release -> main
    Flow {
        #[command(subcommand)]
        sub: FlowCmd,
    },
    /// Generate taskit.toml and Cruxfile for the current workspace
    Init {
        /// Overwrite existing taskit.toml
        #[arg(long)]
        force: bool,
        /// Interactive mode with prompts
        #[arg(long)]
        interactive: bool,
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
    use command::*;
    match cmd {
        Cmd::Fmt { check, affected } => Box::new(Fmt { check, affected }),
        Cmd::Lint {
            crate_name,
            affected,
            continue_on_error,
        } => Box::new(Lint {
            crate_name,
            affected,
            continue_on_error,
        }),
        Cmd::Test {
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
        Cmd::Coverage {
            crate_name,
            threshold,
        } => Box::new(Coverage {
            crate_name,
            threshold,
        }),
        Cmd::CheckProtocolDrift {
            update,
            warn_only,
            hook,
        } => Box::new(CheckProtocolDrift {
            update,
            warn_only,
            hook,
        }),
        Cmd::CheckProtocolSites {
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
        Cmd::Quick => Box::new(Quick),
        Cmd::Ci {
            fail_fast,
            include_network,
        } => Box::new(Ci {
            fail_fast,
            include_network,
        }),
        Cmd::CompileTests => Box::new(CompileTests),
        Cmd::CheckDeps => Box::new(CheckDeps),
        Cmd::CheckFreshness => Box::new(CheckFreshness),
        Cmd::PreCommit => Box::new(PreCommit),
        Cmd::PrePush => Box::new(PrePush),
        Cmd::InstallHooks => Box::new(InstallHooks),
        Cmd::Install => Box::new(Install),
        Cmd::Update { aggressive } => Box::new(Update { aggressive }),
        Cmd::Patch => Box::new(Patch {
            kind: patch::BumpKind::Patch,
        }),
        Cmd::Minor => Box::new(Patch {
            kind: patch::BumpKind::Minor,
        }),
        Cmd::Major => Box::new(Patch {
            kind: patch::BumpKind::Major,
        }),
        Cmd::Audit => Box::new(Audit),
        Cmd::Clean { older_than } => Box::new(Clean { older_than }),
        Cmd::Version => Box::new(Version),
        Cmd::DevSetup => Box::new(DevSetup),
        Cmd::SelfCheck => Box::new(SelfCheck),
        Cmd::SelfTest => Box::new(SelfTest),
        Cmd::UpdateClaudeVersion { version } => Box::new(UpdateClaudeVersion { version }),
        Cmd::Proptest { crate_name } => Box::new(Proptest { crate_name }),
        Cmd::Fuzz { target, duration } => Box::new(Fuzz { target, duration }),
        Cmd::Bench {
            crate_name,
            save_baseline,
        } => Box::new(Bench {
            crate_name,
            save_baseline,
        }),
        Cmd::TestReport => Box::new(TestReport),
        Cmd::SnapshotReview => Box::new(SnapshotReview),
        Cmd::Health { update } => Box::new(Health { update }),
        Cmd::Inspect {
            max_warnings,
            max_todo,
        } => Box::new(Inspect {
            max_warnings,
            max_todo,
        }),
        Cmd::Publish {
            skip_docs,
            allow_dirty,
        } => Box::new(Publish {
            skip_docs,
            allow_dirty,
        }),
        Cmd::Release { tag, notes_file } => Box::new(Release { tag, notes_file }),
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
        Cmd::Init { .. } => unreachable!("Init is handled before dispatch"),
    }
}

fn main() -> miette::Result<()> {
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
