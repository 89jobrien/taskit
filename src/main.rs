use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{env, path::Path};
use taskit_core::config::DEFAULT_COVERAGE_THRESHOLD;
use taskit_core::output_format::OutputFormat;
use taskit_engine::{
    audit, check_deps, check_freshness, ci, clean, dev_setup, fmt, health, hooks, inspect, lint,
    protocol, publish, quick, runner, testing, update_claude, version,
};
use xshell::Shell;

#[derive(Parser)]
#[command(name = "taskit", about = "Config-driven cargo xtask runner")]
struct Cli {
    /// Print commands without executing them
    #[arg(long, global = true)]
    dry_run: bool,
    /// Output format: human (default), json, github, junit
    #[arg(long, global = true, default_value = "human")]
    output: OutputFormat,
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
    /// Install git hooks that delegate to cargo xtask
    InstallHooks,
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
    /// Run xtask's own test suite (hash-cached: skipped when source is unchanged)
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
        /// Maximum allowed clippy warnings (default: 0)
        #[arg(long, default_value_t = 0)]
        max_warnings: usize,
        /// Maximum allowed TODO/FIXME comments (unchecked if omitted)
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Init runs before config loading (taskit.toml may not exist yet)
    if let Cmd::Init { force, interactive } = cli.cmd {
        return taskit_init::run(force, interactive);
    }

    let workspace = taskit_engine::config::load()?;
    let workspace_root = workspace.root.clone();
    env::set_current_dir(&workspace_root)?;
    let config = workspace.config;
    let ws = &config.workspace;
    let proto = config.protocol.as_ref();
    let sh = Shell::new()?;

    runner::set_dry_run(cli.dry_run);
    match cli.cmd {
        Cmd::Fmt { check, affected } => fmt::run(&sh, ws, check, affected),
        Cmd::Lint {
            crate_name,
            affected,
            continue_on_error,
        } => lint::run(&sh, ws, crate_name.as_deref(), affected, continue_on_error),
        Cmd::Test {
            crate_name,
            affected,
            continue_on_error,
            offline,
        } => testing::run::run(
            &sh,
            ws,
            crate_name.as_deref(),
            affected,
            continue_on_error,
            offline,
        ),
        Cmd::Coverage {
            crate_name,
            threshold,
        } => {
            let pkg = crate_name
                .as_deref()
                .or(config.coverage.as_ref().map(|c| c.crate_name.as_str()));
            match pkg {
                Some(name) => testing::coverage::run(&sh, name, threshold),
                None => anyhow::bail!(
                    "no crate specified: use --crate-name or set [coverage].crate_name in taskit.toml"
                ),
            }
        }
        Cmd::CheckProtocolDrift {
            update,
            warn_only,
            hook,
        } => {
            let root = env::current_dir()?;
            protocol::drift::run(&root, proto, update, warn_only, hook)
        }
        Cmd::CheckProtocolSites {
            file,
            pattern,
            expected,
            warn_only,
        } => protocol::sites::run(Path::new(&file), &pattern, expected, warn_only),
        Cmd::Quick => quick::run(&sh, ws),
        Cmd::Ci {
            fail_fast,
            include_network,
        } => ci::run(
            &sh,
            ws,
            proto,
            config.ci.as_ref(),
            config.coverage.as_ref(),
            fail_fast,
            include_network,
            cli.output,
        ),
        Cmd::CompileTests => testing::compile::run(&sh),
        Cmd::CheckDeps => check_deps::run(&sh),
        Cmd::CheckFreshness => check_freshness::run(&sh, proto),
        Cmd::PreCommit => hooks::pre_commit(&sh),
        Cmd::PrePush => hooks::pre_push(&sh, ws, proto, config.coverage.as_ref()),
        Cmd::InstallHooks => hooks::install_hooks(),
        Cmd::Audit => audit::run(&sh),
        Cmd::Clean { older_than } => clean::run(&sh, older_than.as_deref()),
        Cmd::Version => version::run(&sh, ws),
        Cmd::DevSetup => dev_setup::setup(&sh),
        Cmd::SelfCheck => dev_setup::self_check(),
        Cmd::SelfTest => testing::self_test::run(&sh),
        Cmd::UpdateClaudeVersion { version: ver } => update_claude::run(&sh, &ver),
        Cmd::Proptest { crate_name } => testing::proptest::run(&sh, &crate_name),
        Cmd::Fuzz { target, duration } => testing::fuzz::run(&sh, &target, duration),
        Cmd::Bench {
            crate_name,
            save_baseline,
        } => testing::bench::run(&sh, crate_name.as_deref(), save_baseline),
        Cmd::TestReport => testing::report::run(&sh),
        Cmd::SnapshotReview => testing::snapshot::run(&sh),
        Cmd::Health { update } => health::run(&sh, &workspace_root, update),
        Cmd::Inspect {
            max_warnings,
            max_todo,
        } => inspect::run(&sh, max_warnings, max_todo),
        Cmd::Publish {
            skip_docs,
            allow_dirty,
        } => publish::run(&sh, skip_docs, allow_dirty),
        Cmd::Init { .. } => unreachable!("handled above"),
    }
}
