// TODO(audit): 932 lines — split candidate.
use serde::{Deserialize, Serialize};
use std::path::Path;
use taskit_types::error::{TaskitError, TaskitResultExt};
use xshell::{Shell, cmd};

use crate::ctx::Ctx;
use crate::telemetry::{NdjsonStore, latest_metric};
use crate::testing::coverage::{CoverageScope, collect_percent};

const BASELINE_FILE: &str = ".health-baseline.json";
const TELEMETRY_WINDOW_DAYS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Snapshot of health metrics used for regression gating.
pub struct HealthBaseline {
    /// Baseline collection date.
    pub date: String,
    /// Aggregate test counts.
    pub tests: TestCounts,
    /// Aggregate clippy counts.
    pub clippy: ClippyCounts,
    /// Total TODO/FIXME markers in workspace sources.
    pub todo_fixme: usize,
    #[serde(default)]
    /// Aggregate safety-marker counts.
    pub safety: SafetyCounts,
    /// Workspace line-coverage percentage. `None` unless collected with
    /// `--with-coverage` (expensive: compiles with instrumentation).
    #[serde(default)]
    pub coverage: Option<f64>,
    /// Most recent `ci_duration_ms` telemetry reading within the last
    /// `TELEMETRY_WINDOW_DAYS` days. `None` if `taskit ci` hasn't run yet.
    #[serde(default)]
    pub ci_duration_ms: Option<f64>,
    /// Number of workspace crates considered.
    pub crates: usize,
    /// Whether workspace crate versions are aligned.
    pub versions_consistent: bool,
    /// Workspace version string.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Parsed nextest summary totals.
pub struct TestCounts {
    /// Total discovered tests.
    pub total: usize,
    /// Passing tests.
    pub passed: usize,
    /// Failing tests.
    pub failed: usize,
    /// Skipped tests.
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Parsed clippy diagnostic totals.
pub struct ClippyCounts {
    /// Number of clippy warnings.
    pub warnings: usize,
    /// Number of clippy errors.
    pub errors: usize,
}

/// Counts of `.unwrap()`/`.expect()` and `warn!()` call sites in workspace source.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SafetyCounts {
    /// Count of `.unwrap()`/`.expect()` call sites.
    pub unwrap_count: usize,
    /// Count of `warn!()` call sites.
    pub warn_count: usize,
}

/// Collect a fresh health baseline from the workspace.
///
/// `with_coverage` opts into a workspace-wide `cargo llvm-cov` run — skipped
/// by default since it compiles with instrumentation and is materially more
/// expensive than the nextest/clippy passes already in this function.
pub fn collect(ctx: &Ctx, with_coverage: bool) -> Result<HealthBaseline, TaskitError> {
    let sh = &ctx.sh;
    let tests = collect_tests(sh)?;
    let clippy = collect_clippy(sh)?;
    let todo_fixme = count_todo_fixme(sh)?;
    let safety = count_safety(sh)?;
    let coverage = if with_coverage {
        collect_percent(ctx, &CoverageScope::Workspace)?
    } else {
        None
    };
    let store = NdjsonStore::new(ctx.root());
    let ci_duration_ms = latest_metric(&store, "ci_duration_ms", TELEMETRY_WINDOW_DAYS)?;
    let (crate_count, versions_consistent, version) = collect_versions(ctx)?;

    let date = today();

    Ok(HealthBaseline {
        date,
        tests,
        clippy,
        todo_fixme,
        safety,
        coverage,
        ci_duration_ms,
        crates: crate_count,
        versions_consistent,
        version,
    })
}

/// Load an existing baseline from `.health-baseline.json`.
pub fn load_baseline(workspace_root: &Path) -> Result<HealthBaseline, TaskitError> {
    let path = workspace_root.join(BASELINE_FILE);
    let content = std::fs::read_to_string(&path)
        .err_context_with(|| format!("no baseline found at {}", path.display()))?;
    serde_json::from_str(&content).err_context("failed to parse health baseline")
}

/// Write a baseline to `.health-baseline.json`.
pub fn write_baseline(workspace_root: &Path, baseline: &HealthBaseline) -> Result<(), TaskitError> {
    let path = workspace_root.join(BASELINE_FILE);
    let json = serde_json::to_string_pretty(baseline).map_err(TaskitError::other)?;
    std::fs::write(&path, format!("{json}\n")).err_context("failed to write health baseline")
}

/// Compare only the safety counts (`.unwrap()`/`.expect()` and `warn!()`
/// call sites) against a previous baseline. Narrower than [`check`]: skips
/// tests/clippy/coverage/duration, so it stays deterministic and fast enough
/// to run as its own `[ci] steps` gate without requiring a full baseline
/// covering every metric.
/// Returns `Ok(true)` if no regressions, `Ok(false)` if regressions found.
pub fn check_safety(current: &HealthBaseline, previous: &HealthBaseline) -> bool {
    let mut regressed = false;

    taskit_output::taskit_progress!("Safety Gate (baseline: {})", previous.date);
    taskit_output::taskit_progress!("{}", "-".repeat(50));

    regressed |= print_metric(
        "unwrap/expect",
        previous.safety.unwrap_count,
        current.safety.unwrap_count,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "warn!() sites",
        previous.safety.warn_count,
        current.safety.warn_count,
        Direction::LowerIsBetter,
    );

    taskit_output::taskit_progress!("{}", "-".repeat(50));
    if regressed {
        taskit_output::taskit_err!("SAFETY REGRESSION detected");
    } else {
        taskit_output::taskit_ok!("No safety regressions");
    }
    !regressed
}

/// Compare current against a previous baseline and print a report.
/// Returns `Ok(true)` if no regressions, `Ok(false)` if regressions found.
pub fn check(current: &HealthBaseline, previous: &HealthBaseline) -> bool {
    let mut regressed = false;

    taskit_output::taskit_progress!("Health Check (baseline: {})", previous.date);
    taskit_output::taskit_progress!("{}", "-".repeat(50));

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
        "unwrap/expect",
        previous.safety.unwrap_count,
        current.safety.unwrap_count,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "warn!() sites",
        previous.safety.warn_count,
        current.safety.warn_count,
        Direction::LowerIsBetter,
    );
    regressed |= print_metric(
        "Crates",
        previous.crates,
        current.crates,
        Direction::Neutral,
    );
    regressed |= print_coverage_metric(previous.coverage, current.coverage);
    regressed |= print_duration_metric(previous.ci_duration_ms, current.ci_duration_ms);

    if !current.versions_consistent {
        taskit_output::taskit_err!(
            "Versions consistent:  no (was: {})",
            previous.versions_consistent
        );
        regressed = true;
    } else {
        taskit_output::taskit_ok!("Versions consistent:  yes");
    }

    taskit_output::taskit_progress!("{}", "-".repeat(50));
    if regressed {
        taskit_output::taskit_err!("REGRESSION detected");
    } else {
        taskit_output::taskit_ok!("No regressions");
    }

    !regressed
}

/// Run the health subcommand.
///
/// `gate` restricts the regression check to safety counts only (see
/// [`check_safety`]) — intended for use as a lightweight `[ci] steps` entry
/// that doesn't require tracking tests/clippy/coverage in the baseline.
pub fn run(ctx: &Ctx, update: bool, with_coverage: bool, gate: bool) -> Result<(), TaskitError> {
    let workspace_root = ctx.root();
    let current = collect(ctx, with_coverage)?;

    if update {
        write_baseline(workspace_root, &current)?;
        taskit_output::taskit_ok!("Baseline written to {BASELINE_FILE}");
        print_summary(&current);
        return Ok(());
    }

    if gate {
        let previous = load_baseline(workspace_root)?;
        return if check_safety(&current, &previous) {
            Ok(())
        } else {
            Err(TaskitError::other("safety regression detected"))
        };
    }

    match load_baseline(workspace_root) {
        Ok(previous) => {
            if check(&current, &previous) {
                Ok(())
            } else {
                Err(TaskitError::other("health regression detected"))
            }
        }
        Err(_) => {
            taskit_output::taskit_progress!("No existing baseline found. Current health:");
            print_summary(&current);
            taskit_output::taskit_progress!("Run `taskit health --update` to create a baseline.");
            Ok(())
        }
    }
}

fn print_summary(b: &HealthBaseline) {
    taskit_output::taskit_progress!(
        "Tests:       {} total, {} passed, {} failed, {} skipped",
        b.tests.total,
        b.tests.passed,
        b.tests.failed,
        b.tests.skipped
    );
    taskit_output::taskit_progress!(
        "Clippy:      {} warnings, {} errors",
        b.clippy.warnings,
        b.clippy.errors
    );
    taskit_output::taskit_progress!("TODO/FIXME:  {}", b.todo_fixme);
    taskit_output::taskit_progress!(
        "Safety:      {} unwrap/expect, {} warn!() sites",
        b.safety.unwrap_count,
        b.safety.warn_count
    );
    taskit_output::taskit_progress!("Crates:      {}", b.crates);
    match b.coverage {
        Some(pct) => taskit_output::taskit_progress!("Coverage:    {pct:.1}%"),
        None => taskit_output::taskit_progress!("Coverage:    not collected (use --with-coverage)"),
    }
    match b.ci_duration_ms {
        Some(ms) => taskit_output::taskit_progress!("CI duration: {ms:.0}ms"),
        None => taskit_output::taskit_progress!("CI duration: no telemetry yet (run `taskit ci`)"),
    }
    taskit_output::taskit_progress!(
        "Version:     {} (consistent: {})",
        b.version,
        b.versions_consistent
    );
}

// -- Collectors ---------------------------------------------------------------

fn collect_tests(sh: &Shell) -> Result<TestCounts, TaskitError> {
    // Run nextest and parse its output. Nextest exits non-zero on failures,
    // so we capture the output regardless of exit code.
    let output = cmd!(sh, "cargo nextest run --workspace --no-fail-fast")
        .ignore_status()
        .read_stderr()
        .map_err(TaskitError::other)?;
    parse_nextest_summary(&output)
}

fn collect_clippy(sh: &Shell) -> Result<ClippyCounts, TaskitError> {
    let output = cmd!(sh, "cargo clippy --workspace --message-format=json")
        .ignore_status()
        .read()
        .map_err(TaskitError::other)?;
    Ok(parse_clippy_json(&output))
}

fn count_todo_fixme(_sh: &Shell) -> Result<usize, TaskitError> {
    let root = std::env::current_dir().err_context("failed to read current directory")?;
    count_todo_fixme_in_dir(&root)
}

fn count_todo_fixme_in_dir(root: &Path) -> Result<usize, TaskitError> {
    ["crates", "src"].iter().try_fold(0, |total, child| {
        Ok(total + count_todo_fixme_in_tree(&root.join(child))?)
    })
}

fn count_todo_fixme_in_tree(path: &Path) -> Result<usize, TaskitError> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total = 0;
    for entry in
        std::fs::read_dir(path).err_context_with(|| format!("failed to read {}", path.display()))?
    {
        let entry =
            entry.err_context_with(|| format!("failed to read entry in {}", path.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .err_context_with(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            total += count_todo_fixme_in_tree(&path)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let content = std::fs::read_to_string(&path)
                .err_context_with(|| format!("failed to read {}", path.display()))?;
            total += count_todo_fixme_markers(&content);
        }
    }
    Ok(total)
}

fn count_todo_fixme_markers(content: &str) -> usize {
    let mut in_block_comment = false;
    content
        .lines()
        .filter(|line| line_has_todo_fixme_comment(line, &mut in_block_comment))
        .count()
}

/// Finds the byte positions of the first `//` and first `/*` in `s` that fall
/// outside string and character literals.
fn find_comment_markers(s: &str) -> (Option<usize>, Option<usize>) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // b"..." / b'x' / c"..." — advance past the prefix so the next arm handles the quote
            b'b' | b'c' if i + 1 < bytes.len() && matches!(bytes[i + 1], b'"' | b'\'') => {
                i += 1;
            }
            // Raw strings: r"...", r#"..."#, and br"..." (after the `b` prefix is consumed above)
            b'r' if i + 1 < bytes.len() && matches!(bytes[i + 1], b'"' | b'#') => {
                let hash_start = i + 1;
                let hash_count = bytes[hash_start..]
                    .iter()
                    .take_while(|&&b| b == b'#')
                    .count();
                let quote_pos = hash_start + hash_count;
                if quote_pos < bytes.len() && bytes[quote_pos] == b'"' {
                    let mut close = String::from('"');
                    for _ in 0..hash_count {
                        close.push('#');
                    }
                    match s[quote_pos + 1..].find(close.as_str()) {
                        Some(rel) => i = quote_pos + 1 + rel + close.len(),
                        None => return (None, None),
                    }
                } else {
                    i += 1;
                }
            }
            // Regular string literal: "..."
            b'"' => {
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            // Char literal '\n', 'x', etc. — distinguished from lifetimes like 'a
            b'\'' => {
                i += 1;
                if i < bytes.len() && bytes[i] == b'\\' {
                    // Escape sequence: '\n', '\u{0041}', etc.
                    i += 1;
                    if i + 1 < bytes.len() && bytes[i] == b'u' && bytes[i + 1] == b'{' {
                        i += 2;
                        while i < bytes.len() && bytes[i] != b'}' {
                            i += 1;
                        }
                        i += 1; // skip '}'
                    } else {
                        i += 1; // skip single-char escape (e.g. 'n', 't', '\\', '\'')
                    }
                    if i < bytes.len() && bytes[i] == b'\'' {
                        i += 1;
                    }
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    // Single-character literal: 'x'
                    i += 2;
                }
                // else: lifetime like 'a — leave i pointing at the char after '
            }
            // Comment markers
            b'/' if i + 1 < bytes.len() => match bytes[i + 1] {
                b'/' => return (Some(i), None),
                b'*' => return (None, Some(i)),
                _ => i += 1,
            },
            _ => i += 1,
        }
    }
    (None, None)
}

fn line_has_todo_fixme_comment(line: &str, in_block_comment: &mut bool) -> bool {
    extract_todo_fixme_comment(line, in_block_comment).is_some()
}

/// If `line`'s comment content (tracking block-comment state across calls via
/// `in_block_comment`) contains a TODO/FIXME marker, returns the trimmed
/// marker text starting at the keyword (e.g. `"TODO: fix this"`).
pub(crate) fn extract_todo_fixme_comment(
    line: &str,
    in_block_comment: &mut bool,
) -> Option<String> {
    let mut rest = line;
    loop {
        if *in_block_comment {
            if let Some(end) = rest.find("*/") {
                let comment = &rest[..end];
                *in_block_comment = false;
                if let Some(text) = extract_todo_fixme_marker(comment) {
                    return Some(text);
                }
                rest = &rest[end + 2..];
                continue;
            }
            return extract_todo_fixme_marker(rest);
        }

        let (line_comment, block_comment) = find_comment_markers(rest);
        match (line_comment, block_comment) {
            (Some(line_start), Some(block_start)) if line_start < block_start => {
                return extract_todo_fixme_marker(&rest[line_start + 2..]);
            }
            (Some(line_start), None) => {
                return extract_todo_fixme_marker(&rest[line_start + 2..]);
            }
            (_, Some(block_start)) => {
                let comment = &rest[block_start + 2..];
                if let Some(end) = comment.find("*/") {
                    if let Some(text) = extract_todo_fixme_marker(&comment[..end]) {
                        return Some(text);
                    }
                    rest = &comment[end + 2..];
                } else {
                    *in_block_comment = true;
                    return extract_todo_fixme_marker(comment);
                }
            }
            (None, None) => return None,
        }
    }
}

fn extract_todo_fixme_marker(text: &str) -> Option<String> {
    let idx = text.find("TODO").or_else(|| text.find("FIXME"))?;
    Some(text[idx..].trim().to_string())
}

fn count_safety(_sh: &Shell) -> Result<SafetyCounts, TaskitError> {
    let root = std::env::current_dir().err_context("failed to read current directory")?;
    count_safety_in_dir(&root)
}

fn count_safety_in_dir(root: &Path) -> Result<SafetyCounts, TaskitError> {
    ["crates", "src"].iter().try_fold(
        SafetyCounts::default(),
        |mut total, child| -> Result<SafetyCounts, TaskitError> {
            let counts = count_safety_in_tree(&root.join(child))?;
            total.unwrap_count += counts.unwrap_count;
            total.warn_count += counts.warn_count;
            Ok(total)
        },
    )
}

fn count_safety_in_tree(path: &Path) -> Result<SafetyCounts, TaskitError> {
    if !path.exists() {
        return Ok(SafetyCounts::default());
    }

    let mut total = SafetyCounts::default();
    for entry in
        std::fs::read_dir(path).err_context_with(|| format!("failed to read {}", path.display()))?
    {
        let entry =
            entry.err_context_with(|| format!("failed to read entry in {}", path.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .err_context_with(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            let counts = count_safety_in_tree(&path)?;
            total.unwrap_count += counts.unwrap_count;
            total.warn_count += counts.warn_count;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let content = std::fs::read_to_string(&path)
                .err_context_with(|| format!("failed to read {}", path.display()))?;
            let (unwrap_count, warn_count) = count_safety_markers(&content);
            total.unwrap_count += unwrap_count;
            total.warn_count += warn_count;
        }
    }
    Ok(total)
}

/// Count `.unwrap(`/`.expect(` and `warn!(` call sites in `content`, skipping
/// comments and (double-quoted) string literals.
fn count_safety_markers(content: &str) -> (usize, usize) {
    let mut in_block_comment = false;
    let mut unwrap_count = 0;
    let mut warn_count = 0;
    for line in content.lines() {
        let code = code_portion(line, &mut in_block_comment);
        let masked = mask_string_literals(&code);
        unwrap_count += masked.matches(".unwrap(").count() + masked.matches(".expect(").count();
        warn_count += masked.matches("warn!(").count();
    }
    (unwrap_count, warn_count)
}

/// Return the portion of `line` that is actual code, stripping any trailing
/// line/block comment. Tracks block-comment state across calls via `in_block_comment`.
fn code_portion(line: &str, in_block_comment: &mut bool) -> String {
    if *in_block_comment {
        return match line.find("*/") {
            Some(end) => {
                *in_block_comment = false;
                code_portion(&line[end + 2..], in_block_comment)
            }
            None => String::new(),
        };
    }

    let (line_comment, block_comment) = find_comment_markers(line);
    match (line_comment, block_comment) {
        (Some(ls), Some(bs)) if ls < bs => line[..ls].to_string(),
        (Some(ls), None) => line[..ls].to_string(),
        (_, Some(bs)) => {
            let prefix = &line[..bs];
            let rest = &line[bs + 2..];
            match rest.find("*/") {
                Some(end) => format!(
                    "{prefix}{}",
                    code_portion(&rest[end + 2..], in_block_comment)
                ),
                None => {
                    *in_block_comment = true;
                    prefix.to_string()
                }
            }
        }
        (None, None) => line.to_string(),
    }
}

/// Replace the contents of double-quoted string literals with spaces so
/// pattern matching doesn't pick up matches inside string data.
fn mask_string_literals(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            for byte in out.iter_mut().take(i.min(bytes.len())).skip(start) {
                *byte = b' ';
            }
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn collect_versions(ctx: &Ctx) -> Result<(usize, bool, String), TaskitError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .err_context("cargo metadata failed")?;

    let packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .collect();

    let crate_count = packages.len();

    let excluded: std::collections::HashSet<&str> = ctx
        .config
        .workspace
        .crates
        .iter()
        .filter(|c| c.exclude_from_version_check)
        .map(|c| c.pkg_name())
        .collect();

    let all_versions: Vec<(String, String)> = packages
        .iter()
        .map(|p| (p.name.to_string(), p.version.to_string()))
        .collect();

    let (consistent, version) = version_consistency(&all_versions, &excluded);

    Ok((crate_count, consistent, version))
}

/// Given `(pkg_name, version)` pairs for every workspace member, decide
/// whether the versions of the non-excluded members are all equal.
///
/// Returns `(consistent, representative_version)`. The representative
/// version is the first non-excluded member's version (empty string if
/// every member is excluded).
fn version_consistency(
    all_versions: &[(String, String)],
    excluded: &std::collections::HashSet<&str>,
) -> (bool, String) {
    let versions: Vec<&str> = all_versions
        .iter()
        .filter(|(name, _)| !excluded.contains(name.as_str()))
        .map(|(_, v)| v.as_str())
        .collect();

    let version = versions.first().copied().unwrap_or_default().to_string();
    let consistent = versions.iter().all(|v| *v == version);

    (consistent, version)
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
    taskit_output::taskit_progress!("{name:<20} {previous:>5} -> {current:>5} {arrow}{marker}");
    regressed
}

/// Coverage is `Option<f64>` since it's only collected with `--with-coverage`.
/// Only flags a regression when both readings are present and current has
/// dropped by more than a small float-noise tolerance.
fn print_coverage_metric(previous: Option<f64>, current: Option<f64>) -> bool {
    const TOLERANCE: f64 = 0.05;
    match (previous, current) {
        (Some(p), Some(c)) => {
            let arrow = if c > p + TOLERANCE {
                "^"
            } else if c < p - TOLERANCE {
                "v"
            } else {
                "="
            };
            let regressed = c < p - TOLERANCE;
            let marker = if regressed { " REGRESSION" } else { "" };
            taskit_output::taskit_progress!(
                "{:<20} {p:>4.1}% -> {c:>4.1}% {arrow}{marker}",
                "Coverage"
            );
            regressed
        }
        (None, Some(c)) => {
            taskit_output::taskit_progress!("{:<20} (new) -> {c:>4.1}%", "Coverage");
            false
        }
        _ => false,
    }
}

/// CI duration is `Option<f64>` (ms) since it's only populated once `taskit
/// ci` has recorded telemetry. Lower is better; flags a regression when
/// current exceeds previous by more than a relative tolerance (absolute ms
/// tolerances don't scale across pipelines of very different size).
fn print_duration_metric(previous: Option<f64>, current: Option<f64>) -> bool {
    const RELATIVE_TOLERANCE: f64 = 0.10;
    match (previous, current) {
        (Some(p), Some(c)) => {
            let threshold = p * (1.0 + RELATIVE_TOLERANCE);
            let arrow = match c.partial_cmp(&p) {
                Some(std::cmp::Ordering::Greater) => "^",
                Some(std::cmp::Ordering::Less) => "v",
                _ => "=",
            };
            let regressed = c > threshold;
            let marker = if regressed { " REGRESSION" } else { "" };
            taskit_output::taskit_progress!(
                "{:<20} {p:>6.0}ms -> {c:>6.0}ms {arrow}{marker}",
                "CI duration"
            );
            regressed
        }
        (None, Some(c)) => {
            taskit_output::taskit_progress!("{:<20} (new) -> {c:>6.0}ms", "CI duration");
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_with_safety(unwrap_count: usize, warn_count: usize) -> HealthBaseline {
        HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 0,
                errors: 0,
            },
            todo_fixme: 0,
            safety: SafetyCounts {
                unwrap_count,
                warn_count,
            },
            coverage: None,
            ci_duration_ms: None,
            crates: 1,
            versions_consistent: true,
            version: "0.1.0".into(),
        }
    }

    // -- check_safety --

    #[test]
    fn check_safety_no_regression_when_counts_unchanged() {
        let prev = baseline_with_safety(3, 2);
        let current = baseline_with_safety(3, 2);
        assert!(check_safety(&current, &prev));
    }

    #[test]
    fn check_safety_no_regression_when_counts_improve() {
        let prev = baseline_with_safety(5, 4);
        let current = baseline_with_safety(2, 1);
        assert!(check_safety(&current, &prev));
    }

    #[test]
    fn check_safety_regresses_on_unwrap_increase() {
        let prev = baseline_with_safety(3, 2);
        let current = baseline_with_safety(4, 2);
        assert!(!check_safety(&current, &prev));
    }

    #[test]
    fn check_safety_regresses_on_warn_increase() {
        let prev = baseline_with_safety(3, 2);
        let current = baseline_with_safety(3, 3);
        assert!(!check_safety(&current, &prev));
    }

    #[test]
    fn check_safety_ignores_non_safety_regressions() {
        // Tests/clippy/coverage all regress here, but check_safety only
        // looks at unwrap/warn counts, so it should still report no
        // regression.
        let prev = HealthBaseline {
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
            ..baseline_with_safety(3, 2)
        };
        let current = HealthBaseline {
            tests: TestCounts {
                total: 50,
                passed: 40,
                failed: 10,
                skipped: 0,
            },
            clippy: ClippyCounts {
                warnings: 10,
                errors: 1,
            },
            ..baseline_with_safety(3, 2)
        };
        assert!(check_safety(&current, &prev));
    }

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

    // -- count_todo_fixme_markers --
    fn todo_marker() -> String {
        ["TO", "DO"].concat()
    }

    fn fixme_marker() -> String {
        ["FIX", "ME"].concat()
    }

    #[test]
    fn count_todo_fixme_markers_counts_line_doc_inline_and_block_comments() {
        let todo = todo_marker();
        let fixme = fixme_marker();
        let content = [
            format!("// {todo}: line comment"),
            format!("/// {fixme}: doc comment"),
            format!("let x = 1; // {todo}: inline comment"),
            "/*".into(),
            format!(" * {todo}: block comment"),
            " */".into(),
        ]
        .join("\n");
        assert_eq!(count_todo_fixme_markers(&content), 4);
    }

    #[test]
    fn count_todo_fixme_markers_ignores_metric_labels_and_strings() {
        let todo = todo_marker();
        let fixme = fixme_marker();
        let content = [
            format!("let label = \"{todo}/{fixme}\";"),
            format!("taskit_output::taskit_progress!(\"{todo}/{fixme}: {{}}\", count);"),
            format!("let ordinary = \"{fixme} but not a comment\";"),
        ]
        .join("\n");
        assert_eq!(count_todo_fixme_markers(&content), 0);
    }

    #[test]
    fn count_todo_fixme_markers_ignores_comment_markers_inside_string_literals() {
        let todo = todo_marker();
        let fixme = fixme_marker();
        let content = [
            // `//` inside a string literal must not be treated as a comment start
            format!("let s = \"// {todo} inside string\";"),
            // `/*` inside a string literal must not be treated as a block comment start
            format!("let t = \"/* {fixme} inside string */\";"),
            // raw string with `//` marker
            format!("let u = r\"// {todo} raw string\";"),
            // string followed by a real comment with no marker
            "let v = \"// not a marker\"; // plain comment".into(),
        ]
        .join("\n");
        assert_eq!(count_todo_fixme_markers(&content), 0);
    }

    #[test]
    fn count_todo_fixme_in_dir_scans_crates_and_src_rust_files_only() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let crate_src = dir.path().join("crates/foo/src");
        let bin_src = dir.path().join("src");
        std::fs::create_dir_all(&crate_src).expect("crate source dir should be created");
        std::fs::create_dir_all(&bin_src).expect("binary source dir should be created");
        std::fs::write(
            crate_src.join("lib.rs"),
            format!("// {}: crate marker\n", todo_marker()),
        )
        .expect("crate Rust file should be written");
        std::fs::write(
            bin_src.join("main.rs"),
            format!("// {}: binary marker\n", fixme_marker()),
        )
        .expect("binary Rust file should be written");
        std::fs::write(
            crate_src.join("README.md"),
            format!("{}: docs marker\n", todo_marker()),
        )
        .expect("non-Rust file should be written");

        assert_eq!(
            count_todo_fixme_in_dir(dir.path()).expect("count should succeed"),
            2
        );
    }

    // -- count_safety_markers --

    #[test]
    fn count_safety_markers_counts_unwrap_expect_and_warn() {
        let content = [
            "let a = foo().unwrap();",
            "let b = bar().expect(\"bar failed\");",
            "warn!(\"something happened\");",
            "log::warn!(\"also counted\");",
        ]
        .join("\n");
        assert_eq!(count_safety_markers(&content), (2, 2));
    }

    #[test]
    fn count_safety_markers_ignores_comments_and_strings() {
        let content = [
            "// call .unwrap() here later",
            "/* .expect(\"todo\") */",
            "let s = \".unwrap() inside a string, not code\";",
            "let ok = 1 + 1;",
        ]
        .join("\n");
        assert_eq!(count_safety_markers(&content), (0, 0));
    }

    #[test]
    fn count_safety_in_dir_scans_crates_and_src_rust_files_only() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let crate_src = dir.path().join("crates/foo/src");
        let bin_src = dir.path().join("src");
        std::fs::create_dir_all(&crate_src).expect("crate source dir should be created");
        std::fs::create_dir_all(&bin_src).expect("binary source dir should be created");
        std::fs::write(crate_src.join("lib.rs"), "fn f() { g().unwrap(); }\n")
            .expect("crate Rust file should be written");
        std::fs::write(bin_src.join("main.rs"), "fn main() { warn!(\"x\"); }\n")
            .expect("binary Rust file should be written");
        std::fs::write(crate_src.join("README.md"), "call .unwrap() per the docs\n")
            .expect("non-Rust file should be written");

        let counts = count_safety_in_dir(dir.path()).expect("count should succeed");
        assert_eq!(counts.unwrap_count, 1);
        assert_eq!(counts.warn_count, 1);
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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
            safety: SafetyCounts::default(),
            coverage: None,
            ci_duration_ms: None,
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

    // -- version_consistency --

    #[test]
    fn version_consistency_all_equal_is_consistent() {
        let versions = vec![
            ("a".to_string(), "1.0.0".to_string()),
            ("b".to_string(), "1.0.0".to_string()),
        ];
        let excluded = std::collections::HashSet::new();
        let (consistent, version) = version_consistency(&versions, &excluded);
        assert!(consistent);
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn version_consistency_mismatch_is_inconsistent() {
        let versions = vec![
            ("a".to_string(), "1.0.0".to_string()),
            ("xtask".to_string(), "0.1.0".to_string()),
        ];
        let excluded = std::collections::HashSet::new();
        let (consistent, _) = version_consistency(&versions, &excluded);
        assert!(!consistent);
    }

    #[test]
    fn version_consistency_excludes_configured_crate() {
        let versions = vec![
            ("a".to_string(), "1.0.0".to_string()),
            ("b".to_string(), "1.0.0".to_string()),
            ("xtask".to_string(), "0.1.0".to_string()),
        ];
        let excluded: std::collections::HashSet<&str> = ["xtask"].into_iter().collect();
        let (consistent, version) = version_consistency(&versions, &excluded);
        assert!(consistent);
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn version_consistency_all_excluded_returns_empty_version() {
        let versions = vec![("xtask".to_string(), "0.1.0".to_string())];
        let excluded: std::collections::HashSet<&str> = ["xtask"].into_iter().collect();
        let (consistent, version) = version_consistency(&versions, &excluded);
        assert!(consistent);
        assert_eq!(version, "");
    }
}
