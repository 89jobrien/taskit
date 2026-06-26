use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{env, path::Path};
use xshell::Shell;

const DEFAULT_COVERAGE_THRESHOLD: f64 = 80.0;

mod affected;
mod audit;
mod cache;
mod check_deps;
mod check_freshness;
mod ci;
mod clean;
mod config;
mod dev_setup;
mod fmt;
mod hooks;
mod lint;
mod progress;
mod protocol;
mod quick;
mod runner;
mod schema;
mod step;
mod testing;
mod update_claude;
mod util;
mod version;

#[derive(Parser)]
#[command(name = "xtask", about = "Maestro workspace dev tasks")]
struct Cli {
    /// Print commands without executing them
    #[arg(long, global = true)]
    dry_run: bool,
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
    /// Dump or check GraphQL schema
    Schema {
        #[arg(long)]
        check: bool,
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
        #[arg(long, default_value = "maestro-common/src/session.rs")]
        file: String,
        /// Pattern to search for
        #[arg(long, default_value = "CreateSessionRequest {")]
        pattern: String,
        /// Expected count
        #[arg(long, default_value = "4")]
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
    /// Run smoke tests against staging or production
    SmokeTest {
        /// Environment: staging or production
        env: String,
    },
    /// Update pinned Claude Code version
    UpdateClaudeVersion {
        /// Version string (e.g., "2.1.50")
        version: String,
    },
    /// Run conformance/contract tests
    TestConformance,
    /// Run Docker integration tests
    TestDocker {
        #[arg(long)]
        filter: Option<String>,
    },
    /// Run K8s integration tests
    TestK8s {
        #[arg(long)]
        setup: bool,
        #[arg(long)]
        clean: bool,
    },
    /// SmolVM test runner
    TestSmolvm {
        #[command(subcommand)]
        sub: SmolVmCmd,
    },
    /// Run property-based tests
    Proptest {
        #[arg(long, value_name = "CRATE")]
        crate_name: Option<String>,
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
    /// Force-delete e2e-* namespaces stuck in Terminating state
    CleanupE2eNamespaces {
        /// Kubernetes context (default: auto-detect)
        #[arg(long)]
        context: Option<String>,
    },
    /// Run e2e test suite against a live cluster
    TestE2e {
        /// Run cleanup-e2e-namespaces before tests
        #[arg(long)]
        cleanup_first: bool,
        /// Test filter expression (nextest -E)
        #[arg(long)]
        filter: Option<String>,
        /// Parallel jobs (default: 2)
        #[arg(long, default_value = "2")]
        jobs: u32,
        /// Kubernetes context (default: auto-detect)
        #[arg(long)]
        context: Option<String>,
    },
}

#[derive(Subcommand)]
enum SmolVmCmd {
    /// Run SmolVM conformance tests
    Conformance,
    /// Run Linux-gated tests inside SmolVM (Phase 2)
    Linux {
        #[arg(long)]
        crate_name: Option<String>,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Remove stale test-smolvm-* machines
    Cleanup,
}

fn main() -> Result<()> {
    let workspace = config::load()?;
    env::set_current_dir(&workspace.root)?;
    let config = workspace.config;
    let ws = &config.workspace;
    let proto = config.protocol.as_ref();
    let sh = Shell::new()?;

    let cli = Cli::parse();
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
        } => testing::coverage::run(
            &sh,
            crate_name.as_deref().unwrap_or("maestro-api"),
            threshold,
        ),
        Cmd::Schema { check } => schema::run(&sh, check),
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
        } => ci::run(&sh, ws, proto, fail_fast, include_network),
        Cmd::CompileTests => testing::compile::run(&sh),
        Cmd::CheckDeps => check_deps::run(&sh),
        Cmd::CheckFreshness => check_freshness::run(&sh, proto),
        Cmd::PreCommit => hooks::pre_commit(&sh),
        Cmd::PrePush => hooks::pre_push(&sh, ws, proto),
        Cmd::InstallHooks => hooks::install_hooks(),
        Cmd::Audit => audit::run(&sh),
        Cmd::Clean { older_than } => clean::run(&sh, older_than.as_deref()),
        Cmd::Version => version::run(&sh),
        Cmd::DevSetup => dev_setup::setup(&sh),
        Cmd::SelfCheck => dev_setup::self_check(),
        Cmd::SelfTest => testing::self_test::run(&sh),
        Cmd::SmokeTest { env } => testing::smoke::run(&sh, &env),
        Cmd::UpdateClaudeVersion { version: ver } => update_claude::run(&sh, &ver),
        Cmd::TestConformance => testing::conformance::run(&sh),
        Cmd::TestDocker { filter } => testing::docker::run(&sh, filter.as_deref()),
        Cmd::TestK8s { setup, clean } => testing::k8s::run(&sh, setup, clean),
        Cmd::TestSmolvm { sub } => match sub {
            SmolVmCmd::Conformance => testing::smolvm::conformance(&sh),
            SmolVmCmd::Linux { crate_name, args } => {
                testing::smolvm::linux(crate_name.as_deref(), &args)
            }
            SmolVmCmd::Cleanup => testing::smolvm::cleanup(&sh),
        },
        Cmd::Proptest { crate_name } => testing::proptest::run(&sh, crate_name.as_deref()),
        Cmd::Fuzz { target, duration } => testing::fuzz::run(&sh, &target, duration),
        Cmd::Bench {
            crate_name,
            save_baseline,
        } => testing::bench::run(&sh, crate_name.as_deref(), save_baseline),
        Cmd::TestReport => testing::report::run(&sh),
        Cmd::SnapshotReview => testing::snapshot::run(&sh),
        Cmd::CleanupE2eNamespaces { context } => {
            testing::k8s::cleanup_stale_namespaces(&sh, context.as_deref())
        }
        Cmd::TestE2e {
            cleanup_first,
            filter,
            jobs,
            context,
        } => testing::k8s::run_e2e(
            &sh,
            cleanup_first,
            filter.as_deref(),
            jobs,
            context.as_deref(),
        ),
    }
}
