# Plan: TUI Flow Tab + Overview Protocol-Drift Line

## Goal

Implement `docs/designs/2026-08-08-tui-flow-tab-design.md`: add a Flow tab to `taskit-tui`
(pipeline hop status, resumable `flow auto` state, conflict resolver, flow-auto telemetry) and
a protocol-drift status line to the existing Overview tab.

## Architecture

- Crates affected: `taskit-engine` (new data functions, one breaking signature change),
  `taskit-tui` (new tab, new `Snapshot` fields)
- New types: `taskit_engine::flow::{FlowHop, FlowStatusReport}`,
  `taskit_engine::protocol::drift::ProtocolDriftStatus`
- New functions: `taskit_engine::flow::status_report`, `taskit_engine::protocol::drift::check`
- Changed signature: `taskit_engine::flow::merge_with_resolution` returns
  `Result<usize, TaskitError>` instead of `Result<(), TaskitError>`
- Data flow: `taskit-engine` reads git/filesystem/telemetry state → pure data structs →
  `taskit-tui::Snapshot::collect` assembles them each tick → `ui::render_flow` /
  `ui::render_health` render them

## Tech Stack

- Rust edition 2024 (workspace default), no new external dependencies
- `ratatui`/`crossterm` (already a `taskit-tui` dependency) for the new tab's widgets
- `xshell` (already a `taskit-engine` dependency) for `status_report`'s git calls

## Tasks

### Task 1: `protocol::drift::check` — read-only drift status

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/protocol/drift.rs`
**Run**: `cargo nextest run -p taskit-engine -E 'test(check_)'`

1. Write failing tests, appended inside the existing `#[cfg(test)] mod tests` block (after the
   last `compare_lockfiles` test, before the closing `}` of the module):

   ```rust
   // --- check ---

   fn test_ctx(root: &Path, surfaces: Option<Vec<SurfaceEntry>>) -> Ctx {
       use taskit_types::config::{Config, ProtocolConfig};
       use taskit_types::output_format::OutputFormat;
       let protocol = surfaces.map(|surfaces| ProtocolConfig {
           surfaces,
           lockfile: None,
       });
       let config = Config {
           protocol,
           ..Config::default()
       };
       Ctx::new(
           xshell::Shell::new().expect("shell"),
           root.to_path_buf(),
           config,
           false,
           OutputFormat::Human,
       )
   }

   #[test]
   fn check_reports_not_configured_when_no_surfaces() {
       let dir = TempDir::new().expect("tempdir");
       let ctx = test_ctx(dir.path(), None);
       let status = check(&ctx).expect("check should succeed with no surfaces");
       assert!(!status.configured);
       assert!(status.drifted_surfaces.is_empty());
   }

   #[test]
   fn check_reports_configured_with_no_lockfile_yet() {
       let dir = TempDir::new().expect("tempdir");
       fs::write(dir.path().join("types.rs"), "pub struct Foo {}").unwrap();
       let surfaces = vec![SurfaceEntry {
           name: "types".into(),
           path: "types.rs".into(),
       }];
       let ctx = test_ctx(dir.path(), Some(surfaces));
       let status = check(&ctx).expect("check should succeed with no lockfile yet");
       assert!(status.configured);
       assert!(status.drifted_surfaces.is_empty());
   }

   #[test]
   fn check_reports_in_sync_when_hashes_match() {
       let dir = TempDir::new().expect("tempdir");
       fs::write(dir.path().join("types.rs"), "pub struct Foo {}").unwrap();
       let surfaces = vec![SurfaceEntry {
           name: "types".into(),
           path: "types.rs".into(),
       }];
       let lockfile = calculate_lockfile(dir.path(), &surfaces).expect("calculate_lockfile");
       write_lockfile(&dir.path().join(DEFAULT_LOCK_PATH), &lockfile).expect("write_lockfile");

       let ctx = test_ctx(dir.path(), Some(surfaces));
       let status = check(&ctx).expect("check should succeed");
       assert!(status.configured);
       assert!(status.drifted_surfaces.is_empty());
   }

   #[test]
   fn check_reports_drifted_surfaces() {
       let dir = TempDir::new().expect("tempdir");
       fs::write(dir.path().join("types.rs"), "pub struct Foo {}").unwrap();
       let surfaces = vec![SurfaceEntry {
           name: "types".into(),
           path: "types.rs".into(),
       }];
       let lockfile = calculate_lockfile(dir.path(), &surfaces).expect("calculate_lockfile");
       write_lockfile(&dir.path().join(DEFAULT_LOCK_PATH), &lockfile).expect("write_lockfile");

       // Change the surface file after the lockfile was written.
       fs::write(dir.path().join("types.rs"), "pub struct Foo { pub x: u8 }").unwrap();

       let ctx = test_ctx(dir.path(), Some(surfaces));
       let status = check(&ctx).expect("check should succeed");
       assert!(status.configured);
       assert_eq!(status.drifted_surfaces, vec!["types".to_string()]);
   }
   ```

   Run: `cargo nextest run -p taskit-engine -E 'test(check_)'`
   Expected: FAIL (`check` and `ProtocolDriftStatus` do not exist)

2. Implement — insert above `fn calculate_lockfile` (after the `Drift` impl block, before
   `pub fn run`):

   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct ProtocolDriftStatus {
       pub configured: bool,
       pub drifted_surfaces: Vec<String>,
   }

   /// Read-only variant of the protocol-drift check: recomputes current surface
   /// hashes and compares against the persisted lockfile. Never writes the
   /// lockfile. Reports "not configured" (no surfaces) and "no lockfile yet"
   /// via `configured`/`drifted_surfaces` rather than `Err`, so a caller that
   /// polls this every tick (the TUI dashboard) doesn't need to distinguish
   /// those from a real error.
   pub fn check(ctx: &Ctx) -> Result<ProtocolDriftStatus, TaskitError> {
       let root = ctx.root();
       let config = ctx.proto();
       let surfaces: &[SurfaceEntry] = config.map(|c| c.surfaces.as_slice()).unwrap_or(&[]);

       if surfaces.is_empty() {
           return Ok(ProtocolDriftStatus {
               configured: false,
               drifted_surfaces: Vec::new(),
           });
       }

       let lock_rel = config
           .map(|c| c.lockfile_path())
           .unwrap_or(DEFAULT_LOCK_PATH);
       let lock_path = root.join(lock_rel);

       let current = calculate_lockfile(root, surfaces)?;
       let Ok(expected) = read_lockfile(&lock_path) else {
           return Ok(ProtocolDriftStatus {
               configured: true,
               drifted_surfaces: Vec::new(),
           });
       };
       let drift = compare_lockfiles(&expected, &current);

       Ok(ProtocolDriftStatus {
           configured: true,
           drifted_surfaces: drift.into_iter().map(|d| d.name).collect(),
       })
   }
   ```

3. Verify:

   ```
   cargo nextest run -p taskit-engine -E 'test(check_)'  → all green
   cargo clippy -p taskit-engine -- -D warnings           → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-engine): add read-only protocol::drift::check"`

---

### Task 2: `flow::status_report` — pure data variant of `status`

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/flow.rs`
**Run**: `cargo nextest run -p taskit-engine -E 'test(status_report)'`

1. Write failing test, appended in `#[cfg(test)] mod tests` after `flow_config_partial_override`:

   ```rust
   #[test]
   fn status_report_flags_missing_branches_and_computes_ahead_behind() {
       let dir = tempfile::tempdir().expect("tempdir");
       let sh = xshell::Shell::new().expect("shell");
       sh.change_dir(dir.path());
       cmd!(sh, "git init -b main").run().expect("git init");
       cmd!(sh, "git config user.email test@example.com")
           .run()
           .expect("email");
       cmd!(sh, "git config user.name Test").run().expect("name");
       sh.write_file("README.md", "# test\n").expect("write");
       cmd!(sh, "git add README.md").run().expect("add");
       cmd!(sh, "git commit -m init").run().expect("commit");
       cmd!(sh, "git branch develop").run().expect("branch develop");
       // staging/release intentionally not created.

       let ctx = Ctx::new(
           sh,
           dir.path().to_path_buf(),
           taskit_types::config::Config::default(),
           false,
           taskit_types::output_format::OutputFormat::Human,
       );
       let flow = default_flow();

       let report = status_report(&ctx, &flow).expect("status_report should succeed");
       assert_eq!(report.current_branch, "main");
       assert_eq!(report.hops.len(), 4);

       let main_to_develop = &report.hops[0];
       assert_eq!(main_to_develop.from, "main");
       assert_eq!(main_to_develop.to, "develop");
       assert!(main_to_develop.branches_exist);
       assert_eq!(main_to_develop.ahead, 0);
       assert_eq!(main_to_develop.behind, 0);

       let develop_to_staging = &report.hops[1];
       assert!(!develop_to_staging.branches_exist, "staging doesn't exist yet");
   }
   ```

   Run: `cargo nextest run -p taskit-engine -E 'test(status_report)'`
   Expected: FAIL (`status_report` and `FlowHop`/`FlowStatusReport` do not exist)

2. Implement — replace the existing `pub fn status` with the following (insert
   `FlowHop`/`FlowStatusReport`/`status_report` immediately above it):

   ```rust
   #[derive(Debug, Clone, PartialEq)]
   pub struct FlowHop {
       pub from: String,
       pub to: String,
       pub ahead: usize,
       pub behind: usize,
       pub branches_exist: bool,
   }

   #[derive(Debug, Clone, PartialEq)]
   pub struct FlowStatusReport {
       pub current_branch: String,
       /// main→develop→staging→release→main, in that order.
       pub hops: Vec<FlowHop>,
   }

   /// Pure data variant of `status()` — `status()` calls this and prints each
   /// hop, with no output change from before this function existed.
   pub fn status_report(ctx: &Ctx, flow: &FlowConfig) -> Result<FlowStatusReport, TaskitError> {
       let sh = &ctx.sh;
       let main = flow.main_branch();
       let develop = flow.develop_branch();
       let staging = flow.staging_branch();
       let release = flow.release_branch();
       let current_branch = current_branch(sh)?;

       let mut hops = Vec::with_capacity(4);
       for (from, to) in [
           (main, develop),
           (develop, staging),
           (staging, release),
           (release, main),
       ] {
           if !branch_exists(sh, from)? || !branch_exists(sh, to)? {
               hops.push(FlowHop {
                   from: from.to_string(),
                   to: to.to_string(),
                   ahead: 0,
                   behind: 0,
                   branches_exist: false,
               });
               continue;
           }
           let (ahead, behind) = ahead_behind(sh, from, to)?;
           hops.push(FlowHop {
               from: from.to_string(),
               to: to.to_string(),
               ahead,
               behind,
               branches_exist: true,
           });
       }

       Ok(FlowStatusReport {
           current_branch,
           hops,
       })
   }

   pub fn status(ctx: &Ctx, flow: &FlowConfig) -> Result<(), TaskitError> {
       let report = status_report(ctx, flow)?;

       taskit_output::taskit_progress!(
           "Flow status (current branch: {})",
           report.current_branch
       );
       taskit_output::taskit_progress!("");

       for hop in &report.hops {
           if !hop.branches_exist {
               taskit_output::taskit_progress!("{} -> {}: (branch missing)", hop.from, hop.to);
           } else {
               taskit_output::taskit_progress!(
                   "{} -> {}: {} ahead, {} behind",
                   hop.from,
                   hop.to,
                   hop.ahead,
                   hop.behind
               );
           }
       }
       Ok(())
   }
   ```

3. Verify:

   ```
   cargo nextest run -p taskit-engine -E 'test(status_report)'  → all green
   cargo nextest run -p taskit-engine -E 'test(flow_status_shows_all_branches)'  → still green (unchanged CLI output)
   cargo clippy -p taskit-engine -- -D warnings                  → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "refactor(taskit-engine): extract flow::status_report from status"`

---

### Task 3: `merge_with_resolution` returns conflict count

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/flow.rs`
**Run**: `cargo nextest run -p taskit-engine -E 'test(merge_with_resolution)'`

1. Write failing tests, appended in `#[cfg(test)] mod tests` after
   `conflict_resolver_fake_escalates`:

   ```rust
   #[test]
   fn merge_with_resolution_fast_path_returns_zero_conflicts() {
       let (_dir, ctx, _flow) = crate::flow::tests_support::merge_test_repo();
       // No conflict: nothing changed on `staging` relative to `release` in the
       // fast-forward-free merge, so the merge succeeds without touching the
       // resolver.
       let count = merge_with_resolution(&ctx, "staging", "flow: fast path", &AlwaysResolve)
           .expect("fast-path merge should succeed");
       assert_eq!(count, 0);
   }
   ```

   Run: `cargo nextest run -p taskit-engine -E 'test(merge_with_resolution_fast_path)'`
   Expected: FAIL — `crate::flow::tests_support` does not exist yet (added in step 2), and even
   once it compiles the test fails because `merge_with_resolution` currently returns `()`, not
   a comparable `usize`.

   Note: this task reuses the existing integration coverage in
   `crates/taskit-engine/tests/flow_integration.rs` for the *conflict* path (that file already
   has `merge_with_resolution_fast_path_no_conflict` and
   `merge_with_resolution_resolver_resolves_conflict` exercising both branches against a real
   git repo) — this task only needs a small `tests_support` helper so the new unit test above
   doesn't duplicate that repo-setup logic. Add it directly above `mod tests` in `flow.rs`:

   ```rust
   #[cfg(test)]
   pub(crate) mod tests_support {
       use super::*;
       use taskit_types::config::Config;
       use taskit_types::output_format::OutputFormat;

       /// Minimal repo with `main`/`staging`/`release`, `staging` one commit
       /// ahead — enough for a fast-path (no-conflict) merge.
       pub(crate) fn merge_test_repo() -> (tempfile::TempDir, Ctx, FlowConfig) {
           let dir = tempfile::tempdir().expect("tempdir");
           let sh = Shell::new().expect("shell");
           sh.change_dir(dir.path());
           cmd!(sh, "git init -b main").run().expect("git init");
           cmd!(sh, "git config user.email test@example.com")
               .run()
               .expect("email");
           cmd!(sh, "git config user.name Test").run().expect("name");
           sh.write_file("README.md", "# test\n").expect("write");
           cmd!(sh, "git add README.md").run().expect("add");
           cmd!(sh, "git commit -m init").run().expect("commit");
           cmd!(sh, "git branch staging").run().expect("branch staging");
           cmd!(sh, "git branch release").run().expect("branch release");
           cmd!(sh, "git checkout staging").run().expect("checkout staging");
           sh.write_file("feature.txt", "feature\n").expect("write");
           cmd!(sh, "git add feature.txt").run().expect("add");
           cmd!(sh, "git commit -m feat").run().expect("commit");
           cmd!(sh, "git checkout release").run().expect("checkout release");

           let ctx = Ctx::new(
               sh,
               dir.path().to_path_buf(),
               Config::default(),
               false,
               OutputFormat::Human,
           );
           (dir, ctx, FlowConfig::default())
       }
   }
   ```

2. Implement — change `merge_with_resolution`'s signature and the two early `Ok(())` returns
   plus the final commit mapping:

   ```rust
   pub fn merge_with_resolution(
       ctx: &Ctx,
       branch: &str,
       message: &str,
       resolver: &dyn ConflictResolver,
   ) -> Result<usize, TaskitError> {
       let sh = &ctx.sh;
       if ctx.dry_run {
           taskit_output::taskit_dry!("git merge --no-ff {branch} -m \"{message}\"");
           return Ok(0);
       }
       let output = cmd!(sh, "git merge --no-ff {branch} -m {message}")
           .quiet()
           .ignore_status()
           .output()
           .map_err(TaskitError::other)?;
       if output.status.success() {
           return Ok(0);
       }
       let porcelain = cmd!(sh, "git status --porcelain")
           .read()
           .map_err(TaskitError::other)?;
       let paths = parse_conflict_paths(&porcelain);
       if paths.is_empty() {
           let stderr = String::from_utf8_lossy(&output.stderr);
           return Err(FlowError::MergeFailed {
               reason: stderr.trim().to_string(),
           }
           .into());
       }
       let files: Vec<ConflictFile> = paths
           .iter()
           .map(|p| read_conflict_file(sh, p))
           .collect::<Result<_, _>>()?;
       let resolved = resolver.resolve(&files)?;
       let conflict_count = resolved.len();
       for r in &resolved {
           let abs_path = ctx.root.join(&r.path);
           std::fs::write(&abs_path, &r.content).map_err(TaskitError::other)?;
           let path = &r.path;
           cmd!(sh, "git add {path}")
               .run()
               .map_err(TaskitError::other)?;
       }
       cmd!(sh, "git commit --no-edit")
           .run()
           .map(|()| conflict_count)
           .map_err(|e| {
               FlowError::MergeFailed {
                   reason: e.to_string(),
               }
               .into()
           })
   }
   ```

   Update the 4 call sites inside `auto_with_ci` to use `?` as before — they still compile
   unchanged at this step since the return value isn't consumed yet (that's Task 4).

3. Verify:

   ```
   cargo nextest run -p taskit-engine -E 'test(merge_with_resolution)'  → all green
   cargo nextest run -p taskit-engine --test flow_integration            → all green (existing
                                                                            is_ok()/is_err()/Err
                                                                            assertions compile
                                                                            unchanged)
   cargo clippy -p taskit-engine -- -D warnings                          → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-engine): merge_with_resolution returns resolved conflict count"`

---

### Task 4: `flow::auto_with_ci` emits `flow_auto_*` telemetry

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/flow.rs`
**Run**: `cargo nextest run -p taskit-engine -E 'test(auto_ci_)'`

1. Extend the two existing tests (`auto_ci_failure_returns_ci_failed_and_stays_on_release`,
   `auto_ci_pass_completes_finish`) to assert telemetry was written. Add at the end of
   `auto_ci_failure_returns_ci_failed_and_stays_on_release` (after the existing branch
   assertion):

   ```rust
       let store = crate::telemetry::NdjsonStore::new(ctx.root.clone());
       let records = store.load_window(7).expect("load telemetry window");
       let last = records
           .last()
           .expect("a flow_auto telemetry record should have been written");
       let metric = |name: &str| {
           last.metrics
               .iter()
               .find(|m| m.name == name)
               .unwrap_or_else(|| panic!("missing metric {name}"))
               .value
       };
       assert_eq!(metric("flow_auto_result"), 0.0);
       assert!(metric("flow_auto_duration_ms") >= 0.0);
   ```

   And at the end of `auto_ci_pass_completes_finish`:

   ```rust
       let store = crate::telemetry::NdjsonStore::new(ctx.root.clone());
       let records = store.load_window(7).expect("load telemetry window");
       let last = records
           .last()
           .expect("a flow_auto telemetry record should have been written");
       let metric = |name: &str| {
           last.metrics
               .iter()
               .find(|m| m.name == name)
               .unwrap_or_else(|| panic!("missing metric {name}"))
               .value
       };
       assert_eq!(metric("flow_auto_result"), 1.0);
       assert!(metric("flow_auto_duration_ms") >= 0.0);
       assert_eq!(metric("flow_auto_conflicts"), 0.0);
   ```

   Run: `cargo nextest run -p taskit-engine -E 'test(auto_ci_)'`
   Expected: FAIL (no `flow_auto_*` metrics recorded yet)

2. Implement — in `auto_with_ci`, add timing/accumulator setup right after the `use` lines,
   accumulate conflict counts at each `merge_with_resolution` call, and record telemetry at
   both terminal points:

   ```rust
   pub fn auto_with_ci(
       ctx: &Ctx,
       flow: &FlowConfig,
       resolver: &dyn ConflictResolver,
       run_ci: impl Fn(&Ctx) -> taskit_types::step::PipelineOutcome,
   ) -> Result<(), TaskitError> {
       use taskit_types::flow_state::{FlowPhase, FlowState};
       use taskit_types::step::StepStatus;

       let start = std::time::Instant::now();
       let mut conflicts_resolved: usize = 0;

       let sh = &ctx.sh;
       let develop = flow.develop_branch();
       let staging = flow.staging_branch();
       let release = flow.release_branch();
       let main = flow.main_branch();

       // Load any persisted state from a prior interrupted run.
       let saved = crate::flow_state_store::load(&ctx.root);

       let resume_phase = saved.as_ref().map(|s| &s.phase);
       if let Some(phase) = resume_phase {
           taskit_output::taskit_progress!("auto: resuming from {phase:?}");
       }

       // ── Phase 1: Promoting ────────────────────────────────────────────────

       if resume_phase.is_none() || resume_phase == Some(&FlowPhase::Promoting) {
           if resume_phase.is_none() {
               // Fresh run — validate preconditions.
               require_branch(sh, develop)?;
               require_clean(sh, develop)?;
               require_branch_exists(sh, staging)?;
               require_branch_exists(sh, release)?;
               require_branch_exists(sh, main)?;

               let state = FlowState::promoting(staging, release, main);
               if !ctx.dry_run {
                   crate::flow_state_store::save(&ctx.root, &state)?;
               }
           }

           taskit_output::taskit_progress!("auto: promoting {develop} → {staging}");
           checkout(ctx, staging)?;
           conflicts_resolved += merge_with_resolution(
               ctx,
               develop,
               &format!("flow: promote {develop} into {staging}"),
               resolver,
           )?;

           taskit_output::taskit_progress!("auto: staging {staging} → {release}");
           checkout(ctx, release)?;
           conflicts_resolved += merge_with_resolution(
               ctx,
               staging,
               &format!("flow: stage {staging} into {release}"),
               resolver,
           )?;

           // Advance state to CiGate.
           if !ctx.dry_run {
               let state = FlowState {
                   phase: FlowPhase::CiGate,
                   staging: staging.to_string(),
                   release: release.to_string(),
                   main: main.to_string(),
                   merge_sha: None,
                   failed_steps: vec![],
               };
               crate::flow_state_store::save(&ctx.root, &state)?;
           }
       }

       // ── Phase 2: CI gate ─────────────────────────────────────────────────

       if resume_phase.is_none()
           || resume_phase == Some(&FlowPhase::Promoting)
           || resume_phase == Some(&FlowPhase::CiGate)
       {
           taskit_output::taskit_progress!("auto: running CI on {release}");
           let outcome = run_ci(ctx);
           if !outcome.passed {
               let failed: Vec<String> = outcome
                   .results
                   .iter()
                   .filter(|s| s.status == StepStatus::Fail)
                   .map(|s| s.name.clone())
                   .collect();
               // Persist failure state so the user can re-run after fixing CI.
               if !ctx.dry_run {
                   let state = FlowState {
                       phase: FlowPhase::CiGate,
                       staging: staging.to_string(),
                       release: release.to_string(),
                       main: main.to_string(),
                       merge_sha: None,
                       failed_steps: failed.clone(),
                   };
                   crate::flow_state_store::save(&ctx.root, &state)?;
               }
               taskit_output::taskit_err!(
                   "auto: CI failed on {release} — staying on {release} for investigation"
               );
               let _ = crate::telemetry::record(
                   ctx,
                   &[
                       ("flow_auto_duration_ms", start.elapsed().as_millis() as f64),
                       ("flow_auto_result", 0.0),
                       ("flow_auto_conflicts", conflicts_resolved as f64),
                   ],
               );
               return Err(FlowError::CiFailed { failed }.into());
           }
           taskit_output::taskit_ok!("auto: CI passed on {release}");

           // Advance to Finishing.
           if !ctx.dry_run {
               let state = FlowState {
                   phase: FlowPhase::Finishing,
                   staging: staging.to_string(),
                   release: release.to_string(),
                   main: main.to_string(),
                   merge_sha: None,
                   failed_steps: vec![],
               };
               crate::flow_state_store::save(&ctx.root, &state)?;
           }
       }

       // ── Phase 3: Finishing ────────────────────────────────────────────────

       taskit_output::taskit_progress!("auto: finishing {release} → {main}");
       checkout(ctx, main)?;
       conflicts_resolved += merge_with_resolution(
           ctx,
           release,
           &format!("flow: finish {release} into {main}"),
           resolver,
       )?;

       taskit_output::taskit_progress!("auto: syncing {main} → {develop}");
       checkout(ctx, develop)?;
       conflicts_resolved +=
           merge_with_resolution(ctx, main, &sync_commit_message(main, develop), resolver)?;

       // Success — clear the state file.
       if !ctx.dry_run {
           crate::flow_state_store::clear(&ctx.root)?;
       }

       let _ = crate::telemetry::record(
           ctx,
           &[
               ("flow_auto_duration_ms", start.elapsed().as_millis() as f64),
               ("flow_auto_result", 1.0),
               ("flow_auto_conflicts", conflicts_resolved as f64),
           ],
       );

       taskit_output::taskit_ok!("auto: done. {develop} is in sync with {main}.");
       Ok(())
   }
   ```

3. Verify:

   ```
   cargo nextest run -p taskit-engine -E 'test(auto_ci_)'  → all green
   cargo nextest run -p taskit-engine                       → all green (full crate, catches
                                                                any missed call site)
   cargo clippy -p taskit-engine -- -D warnings              → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-engine): emit flow_auto_* telemetry from auto_with_ci"`

---

### Task 5: `taskit-tui::Snapshot` gains flow + protocol-drift fields

**Crate**: `taskit-tui`
**File(s)**: `crates/taskit-tui/src/snapshot.rs`
**Run**: `cargo nextest run -p taskit-tui -E 'test(snapshot::)'`

1. Update the existing `from_parts`-based tests to the new signature (they currently call
   `Snapshot::from_parts(None, &records)` / `Snapshot::from_parts(baseline, &records)` with 2
   args). Replace each call site in the 4 existing tests
   (`no_records_yields_empty_snapshot`, `single_record_has_no_drift_baseline_yet`,
   `multiple_records_compute_drift_against_prior_readings`,
   `duration_history_caps_at_sparkline_points`) with:

   ```rust
   let snapshot = Snapshot::from_parts(
       None, // or `baseline` where the original passed it
       &records,
       None,
       None,
       ConflictResolverKind::default(),
       None,
   );
   ```

   and add one new test after `duration_history_caps_at_sparkline_points`:

   ```rust
   #[test]
   fn flow_auto_history_derived_same_way_as_ci_history() {
       let records = vec![
           record(
               "t1",
               &[
                   ("flow_auto_duration_ms", 1000.0),
                   ("flow_auto_result", 1.0),
                   ("flow_auto_conflicts", 2.0),
               ],
           ),
           record(
               "t2",
               &[
                   ("flow_auto_duration_ms", 2000.0),
                   ("flow_auto_result", 0.0),
                   ("flow_auto_conflicts", 0.0),
               ],
           ),
       ];
       let snapshot = Snapshot::from_parts(
           None,
           &records,
           None,
           None,
           ConflictResolverKind::default(),
           None,
       );
       assert_eq!(snapshot.flow_auto_duration_history, vec![1000, 2000]);
       assert_eq!(snapshot.flow_auto_result_history, vec![1, 0]);
       assert_eq!(snapshot.flow_auto_conflicts_last, Some(0));
   }
   ```

   Run: `cargo nextest run -p taskit-tui -E 'test(snapshot::)'`
   Expected: FAIL — `from_parts` doesn't accept 6 args yet, new fields don't exist

2. Implement — replace the full file contents of `crates/taskit-tui/src/snapshot.rs` with:

   ```rust
   //! Read-only, point-in-time view of workspace health for rendering.
   //!
   //! Collection only reads persisted state (the health baseline file,
   //! telemetry NDJSON, flow state file, and git/protocol-surface reads that
   //! never shell out to clippy/nextest/cargo) — so it's cheap enough to
   //! re-run on every dashboard tick.

   use taskit_engine::ctx::Ctx;
   use taskit_engine::drift::{self, DriftReport};
   use taskit_engine::flow::{self, FlowStatusReport};
   use taskit_engine::flow_state_store;
   use taskit_engine::health::{self, HealthBaseline};
   use taskit_engine::protocol::drift::{self as protocol_drift, ProtocolDriftStatus};
   use taskit_engine::telemetry::{NdjsonStore, TelemetryRecord, TelemetryStore};
   use taskit_types::config::ConflictResolverKind;
   use taskit_types::flow_state::FlowState;

   const CI_DURATION_METRIC: &str = "ci_duration_ms";
   const CI_PASSED_METRIC: &str = "ci_passed";
   const FLOW_AUTO_DURATION_METRIC: &str = "flow_auto_duration_ms";
   const FLOW_AUTO_RESULT_METRIC: &str = "flow_auto_result";
   const FLOW_AUTO_CONFLICTS_METRIC: &str = "flow_auto_conflicts";
   const DRIFT_WINDOW_DAYS: u64 = 7;
   const SPARKLINE_POINTS: usize = 30;

   pub struct Snapshot {
       pub refreshed_at: String,
       pub baseline: Option<HealthBaseline>,
       pub ci_run_count: usize,
       pub last_ci_passed: Option<bool>,
       pub ci_duration_drift: Option<DriftReport>,
       /// Raw `ci_duration_ms` readings, oldest first, capped to the last
       /// [`SPARKLINE_POINTS`] — feeds the dashboard's `Sparkline` widget.
       pub ci_duration_history: Vec<u64>,
       /// Raw `ci_passed` readings (0.0/1.0), oldest first, capped to the last
       /// [`SPARKLINE_POINTS`] — feeds the pass/fail trend strip.
       pub ci_passed_history: Vec<u64>,
       /// Full telemetry records in the drift window, oldest first — feeds the
       /// scrollable CI History tab.
       pub records: Vec<TelemetryRecord>,
       /// Git-flow pipeline hop status (main→develop→staging→release→main).
       pub flow_status: Option<FlowStatusReport>,
       /// Resumable `flow auto` state, if a run was interrupted mid-pipeline.
       pub flow_state: Option<FlowState>,
       pub flow_conflict_resolver: ConflictResolverKind,
       /// Raw `flow_auto_duration_ms` readings, oldest first, capped like
       /// `ci_duration_history`.
       pub flow_auto_duration_history: Vec<u64>,
       /// Raw `flow_auto_result` readings (0/1), oldest first, capped like
       /// `ci_passed_history`.
       pub flow_auto_result_history: Vec<u64>,
       pub flow_auto_conflicts_last: Option<u64>,
       pub protocol_drift: Option<ProtocolDriftStatus>,
   }

   impl Snapshot {
       pub fn collect(ctx: &Ctx) -> Self {
           let baseline = health::load_baseline(ctx.root()).ok();
           let store = NdjsonStore::new(ctx.root());
           let records = store.load_window(DRIFT_WINDOW_DAYS).unwrap_or_default();

           let flow_config = ctx.config.flow.clone().unwrap_or_default();
           let flow_status = flow::status_report(ctx, &flow_config).ok();
           let flow_state = flow_state_store::load(ctx.root());
           let flow_conflict_resolver = flow_config.conflict_resolver;

           let drift_status = protocol_drift::check(ctx).ok();

           Self::from_parts(
               baseline,
               &records,
               flow_status,
               flow_state,
               flow_conflict_resolver,
               drift_status,
           )
       }

       #[allow(clippy::too_many_arguments)]
       fn from_parts(
           baseline: Option<HealthBaseline>,
           records: &[TelemetryRecord],
           flow_status: Option<FlowStatusReport>,
           flow_state: Option<FlowState>,
           flow_conflict_resolver: ConflictResolverKind,
           protocol_drift: Option<ProtocolDriftStatus>,
       ) -> Self {
           let metric_values = |name: &str| -> Vec<f64> {
               records
                   .iter()
                   .flat_map(|r| r.metrics.iter())
                   .filter(|m| m.name == name)
                   .map(|m| m.value)
                   .collect()
           };

           let ci_durations = metric_values(CI_DURATION_METRIC);
           let ci_passed = metric_values(CI_PASSED_METRIC);
           let flow_auto_durations = metric_values(FLOW_AUTO_DURATION_METRIC);
           let flow_auto_results = metric_values(FLOW_AUTO_RESULT_METRIC);
           let flow_auto_conflicts = metric_values(FLOW_AUTO_CONFLICTS_METRIC);

           let ci_duration_drift = ci_durations.split_last().and_then(|(&current, base)| {
               if base.is_empty() {
                   None
               } else {
                   Some(drift::analyze(base, current))
               }
           });
           let last_ci_passed = ci_passed.last().map(|&v| v >= 1.0);
           let recent = |values: &[f64]| -> Vec<u64> {
               values
                   .iter()
                   .rev()
                   .take(SPARKLINE_POINTS)
                   .rev()
                   .map(|&v| v.round() as u64)
                   .collect()
           };
           let ci_duration_history = recent(&ci_durations);
           let ci_passed_history = recent(&ci_passed);
           let flow_auto_duration_history = recent(&flow_auto_durations);
           let flow_auto_result_history = recent(&flow_auto_results);
           let flow_auto_conflicts_last = flow_auto_conflicts.last().map(|&v| v.round() as u64);

           Self {
               refreshed_at: now_hms(),
               baseline,
               ci_run_count: ci_passed.len(),
               last_ci_passed,
               ci_duration_drift,
               ci_duration_history,
               ci_passed_history,
               records: records.to_vec(),
               flow_status,
               flow_state,
               flow_conflict_resolver,
               flow_auto_duration_history,
               flow_auto_result_history,
               flow_auto_conflicts_last,
               protocol_drift,
           }
       }
   }

   fn now_hms() -> String {
       std::process::Command::new("date")
           .arg("+%H:%M:%S")
           .output()
           .ok()
           .and_then(|o| String::from_utf8(o.stdout).ok())
           .map(|s| s.trim().to_string())
           .unwrap_or_default()
   }

   #[cfg(test)]
   mod tests {
       use super::*;
       use taskit_engine::telemetry::MetricPoint;

       fn record(ts: &str, metrics: &[(&str, f64)]) -> TelemetryRecord {
           TelemetryRecord {
               timestamp: ts.into(),
               git_sha: None,
               metrics: metrics
                   .iter()
                   .map(|(name, value)| MetricPoint {
                       name: (*name).to_string(),
                       value: *value,
                   })
                   .collect(),
           }
       }

       #[test]
       fn no_records_yields_empty_snapshot() {
           let snapshot = Snapshot::from_parts(
               None,
               &[],
               None,
               None,
               ConflictResolverKind::default(),
               None,
           );
           assert!(snapshot.baseline.is_none());
           assert_eq!(snapshot.ci_run_count, 0);
           assert!(snapshot.last_ci_passed.is_none());
           assert!(snapshot.ci_duration_drift.is_none());
           assert!(snapshot.records.is_empty());
           assert!(snapshot.flow_status.is_none());
           assert!(snapshot.flow_state.is_none());
           assert!(snapshot.protocol_drift.is_none());
       }

       #[test]
       fn single_record_has_no_drift_baseline_yet() {
           let records = vec![record(
               "t1",
               &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
           )];
           let snapshot = Snapshot::from_parts(
               None,
               &records,
               None,
               None,
               ConflictResolverKind::default(),
               None,
           );
           assert_eq!(snapshot.ci_run_count, 1);
           assert_eq!(snapshot.last_ci_passed, Some(true));
           assert!(snapshot.ci_duration_drift.is_none());
           assert_eq!(snapshot.records.len(), 1);
       }

       #[test]
       fn multiple_records_compute_drift_against_prior_readings() {
           let records = vec![
               record(
                   "t1",
                   &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
               ),
               record(
                   "t2",
                   &[(CI_DURATION_METRIC, 100.0), (CI_PASSED_METRIC, 1.0)],
               ),
               record(
                   "t3",
                   &[(CI_DURATION_METRIC, 500.0), (CI_PASSED_METRIC, 0.0)],
               ),
           ];
           let snapshot = Snapshot::from_parts(
               None,
               &records,
               None,
               None,
               ConflictResolverKind::default(),
               None,
           );
           assert_eq!(snapshot.ci_run_count, 3);
           assert_eq!(snapshot.last_ci_passed, Some(false));
           let drift = snapshot
               .ci_duration_drift
               .expect("drift should be computed");
           assert!(drift.regressed, "500 vs baseline of 100s should regress");
           assert_eq!(snapshot.ci_duration_history, vec![100, 100, 500]);
           assert_eq!(snapshot.ci_passed_history, vec![1, 1, 0]);
       }

       #[test]
       fn duration_history_caps_at_sparkline_points() {
           let records: Vec<TelemetryRecord> = (0..(SPARKLINE_POINTS + 10))
               .map(|i| record("t", &[(CI_DURATION_METRIC, i as f64)]))
               .collect();
           let snapshot = Snapshot::from_parts(
               None,
               &records,
               None,
               None,
               ConflictResolverKind::default(),
               None,
           );
           assert_eq!(snapshot.ci_duration_history.len(), SPARKLINE_POINTS);
           // Oldest-first, capped to the most recent SPARKLINE_POINTS readings.
           assert_eq!(snapshot.ci_duration_history.first(), Some(&10));
           assert_eq!(
               snapshot.ci_duration_history.last(),
               Some(&((SPARKLINE_POINTS + 9) as u64))
           );
           assert_eq!(snapshot.records.len(), SPARKLINE_POINTS + 10);
       }

       #[test]
       fn flow_auto_history_derived_same_way_as_ci_history() {
           let records = vec![
               record(
                   "t1",
                   &[
                       ("flow_auto_duration_ms", 1000.0),
                       ("flow_auto_result", 1.0),
                       ("flow_auto_conflicts", 2.0),
                   ],
               ),
               record(
                   "t2",
                   &[
                       ("flow_auto_duration_ms", 2000.0),
                       ("flow_auto_result", 0.0),
                       ("flow_auto_conflicts", 0.0),
                   ],
               ),
           ];
           let snapshot = Snapshot::from_parts(
               None,
               &records,
               None,
               None,
               ConflictResolverKind::default(),
               None,
           );
           assert_eq!(snapshot.flow_auto_duration_history, vec![1000, 2000]);
           assert_eq!(snapshot.flow_auto_result_history, vec![1, 0]);
           assert_eq!(snapshot.flow_auto_conflicts_last, Some(0));
       }
   }
   ```

3. Verify:

   ```
   cargo nextest run -p taskit-tui -E 'test(snapshot::)'  → all green
   cargo clippy -p taskit-tui -- -D warnings                → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-tui): add flow and protocol-drift fields to Snapshot"`

---

### Task 6: `taskit-tui::App` gains `Tab::Flow`

**Crate**: `taskit-tui`
**File(s)**: `crates/taskit-tui/src/app.rs`
**Run**: `cargo nextest run -p taskit-tui -E 'test(app::)'`

1. Update the existing `snapshot_with_records` test helper (it constructs `Snapshot` via
   struct literal, which now needs the 6 new fields from Task 5) and the two tab-cycling tests
   that hardcode a 3-tab sequence. Replace `snapshot_with_records`:

   ```rust
   fn snapshot_with_records(count: usize) -> Snapshot {
       Snapshot {
           refreshed_at: String::new(),
           baseline: None,
           ci_run_count: 0,
           last_ci_passed: None,
           ci_duration_drift: None,
           ci_duration_history: Vec::new(),
           ci_passed_history: Vec::new(),
           records: (0..count)
               .map(|_| taskit_engine::telemetry::TelemetryRecord {
                   timestamp: String::new(),
                   git_sha: None,
                   metrics: Vec::new(),
               })
               .collect(),
           flow_status: None,
           flow_state: None,
           flow_conflict_resolver: taskit_types::config::ConflictResolverKind::default(),
           flow_auto_duration_history: Vec::new(),
           flow_auto_result_history: Vec::new(),
           flow_auto_conflicts_last: None,
           protocol_drift: None,
       }
   }
   ```

   Replace `next_tab_cycles_forward_and_wraps`:

   ```rust
   #[test]
   fn next_tab_cycles_forward_and_wraps() {
       let mut app = app(Tab::Overview, 0);
       app.next_tab();
       assert_eq!(app.active_tab, Tab::Crates);
       app.next_tab();
       assert_eq!(app.active_tab, Tab::History);
       app.next_tab();
       assert_eq!(app.active_tab, Tab::Flow);
       app.next_tab();
       assert_eq!(app.active_tab, Tab::Overview);
   }
   ```

   Replace `prev_tab_cycles_backward_and_wraps`:

   ```rust
   #[test]
   fn prev_tab_cycles_backward_and_wraps() {
       let mut app = app(Tab::Overview, 0);
       app.prev_tab();
       assert_eq!(app.active_tab, Tab::Flow);
       app.prev_tab();
       assert_eq!(app.active_tab, Tab::History);
       app.prev_tab();
       assert_eq!(app.active_tab, Tab::Crates);
       app.prev_tab();
       assert_eq!(app.active_tab, Tab::Overview);
   }
   ```

   Add a new test after `clamp_scroll_overview_tab_has_no_scrollable_content`:

   ```rust
   #[test]
   fn clamp_scroll_flow_tab_has_no_scrollable_content() {
       let mut app = app(Tab::Flow, 50);
       let snapshot = snapshot_with_records(50);

       app.scroll_bottom();
       app.clamp_scroll(&snapshot);
       assert_eq!(app.scroll, 0, "Flow has no scrollable rows");
   }
   ```

   Run: `cargo nextest run -p taskit-tui -E 'test(app::)'`
   Expected: FAIL (`Tab::Flow` doesn't exist; `Snapshot` struct literal missing fields)

2. Implement — update the `Tab` enum and `max_scroll`:

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum Tab {
       Overview,
       Crates,
       History,
       Flow,
   }

   impl Tab {
       pub const ALL: [Tab; 4] = [Tab::Overview, Tab::Crates, Tab::History, Tab::Flow];

       pub fn title(self) -> &'static str {
           match self {
               Tab::Overview => "Overview",
               Tab::Crates => "Crates",
               Tab::History => "History",
               Tab::Flow => "Flow",
           }
       }
   }
   ```

   And in `max_scroll`:

   ```rust
       fn max_scroll(&self, snapshot: &Snapshot) -> u16 {
           let rows = match self.active_tab {
               Tab::Overview => 0,
               Tab::Crates => self.crate_names.len(),
               Tab::History => snapshot.records.len(),
               Tab::Flow => 0,
           };
           u16::try_from(rows.saturating_sub(1)).unwrap_or(u16::MAX)
       }
   ```

3. Verify:

   ```
   cargo nextest run -p taskit-tui -E 'test(app::)'  → all green
   cargo clippy -p taskit-tui -- -D warnings           → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-tui): add Tab::Flow"`

---

### Task 7: Render the Flow tab

**Crate**: `taskit-tui`
**File(s)**: `crates/taskit-tui/src/ui.rs`
**Run**: `cargo check -p taskit-tui`

`ui.rs` has no existing unit tests (rendering is verified by manual `cargo run` per the
project's UI-testing convention, not automated snapshot tests) — this task is implement-then-
manually-verify rather than TDD, matching how Tasks that added `render_crates`/`render_history`
were done in this crate previously.

1. Add imports at the top of `ui.rs`:

   ```rust
   use taskit_types::config::ConflictResolverKind;
   ```

2. Implement — add `render_flow` and its four sub-panels after `render_history`:

   ```rust
   fn render_flow(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
       let rows = Layout::default()
           .direction(Direction::Vertical)
           .constraints([
               Constraint::Length(3),
               Constraint::Length(6),
               Constraint::Length(5),
               Constraint::Min(0),
           ])
           .split(area);

       render_flow_header(frame, rows[0], snapshot);
       render_flow_hops(frame, rows[1], snapshot);
       render_flow_resume_state(frame, rows[2], snapshot);
       render_flow_auto_telemetry(frame, rows[3], snapshot);
   }

   fn render_flow_header(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
       let block = Block::default().title("Flow").borders(Borders::ALL);
       let current_branch = snapshot
           .flow_status
           .as_ref()
           .map(|s| s.current_branch.as_str())
           .unwrap_or("(unknown)");
       let resolver = match snapshot.flow_conflict_resolver {
           ConflictResolverKind::Baml => "baml",
           ConflictResolverKind::None => "none",
       };
       let lines = vec![Line::from(format!(
           "Current branch: {current_branch}   •   conflict resolver: {resolver}"
       ))];
       frame.render_widget(Paragraph::new(lines).block(block), area);
   }

   fn render_flow_hops(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
       let block = Block::default()
           .title("Pipeline (main → develop → staging → release → main)")
           .borders(Borders::ALL);
       let Some(status) = &snapshot.flow_status else {
           frame.render_widget(
               Paragraph::new("Flow status unavailable (not a git repository?).").block(block),
               area,
           );
           return;
       };
       let lines: Vec<Line> = status
           .hops
           .iter()
           .map(|hop| {
               if !hop.branches_exist {
                   Line::from(Span::styled(
                       format!("{} -> {}: (branch missing)", hop.from, hop.to),
                       Style::default().fg(Color::Yellow),
                   ))
               } else {
                   Line::from(format!(
                       "{} -> {}: {} ahead, {} behind",
                       hop.from, hop.to, hop.ahead, hop.behind
                   ))
               }
           })
           .collect();
       frame.render_widget(Paragraph::new(lines).block(block), area);
   }

   fn render_flow_resume_state(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
       let block = Block::default()
           .title("Resumable State")
           .borders(Borders::ALL);
       let Some(state) = &snapshot.flow_state else {
           frame.render_widget(
               Paragraph::new("No interrupted `flow auto` run — nothing to resume.").block(block),
               area,
           );
           return;
       };
       let mut lines = vec![Line::from(Span::styled(
           format!("Resuming: {:?}", state.phase),
           Style::default().fg(Color::Yellow),
       ))];
       lines.push(Line::from(state.hint()));
       if !state.failed_steps.is_empty() {
           lines.push(Line::from(format!(
               "Failed steps: {}",
               state.failed_steps.join(", ")
           )));
       }
       frame.render_widget(Paragraph::new(lines).block(block), area);
   }

   fn render_flow_auto_telemetry(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
       let block = Block::default()
           .title("flow auto runs (7d)")
           .borders(Borders::ALL);
       if snapshot.flow_auto_duration_history.is_empty() {
           frame.render_widget(
               Paragraph::new("No `flow auto` runs recorded yet.").block(block),
               area,
           );
           return;
       }
       let inner = block.inner(area);
       frame.render_widget(block, area);

       let sub_rows = Layout::default()
           .direction(Direction::Vertical)
           .constraints([Constraint::Length(2), Constraint::Min(0)])
           .split(inner);

       let last_result = snapshot.flow_auto_result_history.last().copied();
       let last_conflicts = snapshot.flow_auto_conflicts_last.unwrap_or(0);
       let lines = vec![
           match last_result {
               Some(1) => Line::from(Span::styled(
                   "Last flow auto: PASS",
                   Style::default().fg(Color::Green),
               )),
               Some(_) => Line::from(Span::styled(
                   "Last flow auto: FAIL",
                   Style::default().fg(Color::Red),
               )),
               None => Line::from("Last flow auto: (no data)"),
           },
           Line::from(format!("Last run conflicts resolved: {last_conflicts}")),
       ];
       frame.render_widget(Paragraph::new(lines), sub_rows[0]);

       frame.render_widget(
           Sparkline::default()
               .block(Block::default().title("duration trend"))
               .data(&snapshot.flow_auto_duration_history)
               .style(Style::default().fg(Color::Cyan)),
           sub_rows[1],
       );
   }
   ```

3. Wire it into `render()`'s match and `render_footer`'s nav hint:

   ```rust
       match app.active_tab {
           Tab::Overview => render_overview(frame, chunks[1], snapshot),
           Tab::Crates => render_crates(frame, chunks[1], app),
           Tab::History => render_history(frame, chunks[1], app, snapshot),
           Tab::Flow => render_flow(frame, chunks[1], snapshot),
       }
   ```

   ```rust
       let nav_hint = match app.active_tab {
           Tab::Overview | Tab::Flow => "tab/←→ switch tabs",
           _ => "tab/←→ switch tabs  •  j/k, PgUp/PgDn, g/G scroll",
       };
   ```

4. Verify:

   ```
   cargo check -p taskit-tui              → compiles clean
   cargo clippy -p taskit-tui -- -D warnings  → zero warnings
   cargo run -p taskit -- tui             → manually press Tab 3 times to reach Flow, confirm
                                             it renders without panicking, press Tab again to
                                             wrap back to Overview
   ```

5. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-tui): render the Flow tab"`

---

### Task 8: Overview gains a protocol-drift line

**Crate**: `taskit-tui`
**File(s)**: `crates/taskit-tui/src/ui.rs`
**Run**: `cargo check -p taskit-tui`

Implement-then-manually-verify, same rationale as Task 7 (no existing render-function unit
tests in this file).

1. In `render_health`, extend the final `lines` vec (currently `TODO/FIXME`, `Crates`,
   `Version`, `Baseline`) to append a 5th line built from `snapshot.protocol_drift`:

   ```rust
       let protocol_line = match &snapshot.protocol_drift {
           None => Line::from("Protocol:    (unavailable)"),
           Some(status) if !status.configured => Line::from("Protocol:    not configured"),
           Some(status) if status.drifted_surfaces.is_empty() => Line::from(Span::styled(
               "Protocol:    in sync",
               Style::default().fg(Color::Green),
           )),
           Some(status) => Line::from(Span::styled(
               format!(
                   "Protocol:    DRIFT ({} surface(s): {})",
                   status.drifted_surfaces.len(),
                   status.drifted_surfaces.join(", ")
               ),
               Style::default().fg(Color::Red),
           )),
       };

       let lines = vec![
           Line::from(format!("TODO/FIXME:  {}", b.todo_fixme)),
           Line::from(format!("Crates:      {}", b.crates)),
           Line::from(format!(
               "Version:     {} (consistent: {})",
               b.version, b.versions_consistent
           )),
           Line::from(format!("Baseline:    {}", b.date)),
           protocol_line,
       ];
       frame.render_widget(Paragraph::new(lines), rows[2]);
   ```

   (`rows[2]` already has `Constraint::Min(0)`, so it grows to fit the extra line — no layout
   constraint changes needed.)

2. Verify:

   ```
   cargo check -p taskit-tui                  → compiles clean
   cargo clippy -p taskit-tui -- -D warnings     → zero warnings
   cargo run -p taskit -- tui                 → Overview tab shows a 5th "Protocol:" line
                                                 under Health, no panic with/without
                                                 taskit-protocol.lock present
   ```

3. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit: `git commit -m "feat(taskit-tui): add protocol-drift line to Overview health panel"`

---

### Task 9: Full workspace verification

**Crate**: workspace
**File(s)**: none (verification only)
**Run**: `cargo nextest run --workspace`

1. N/A — no new code, this task confirms the full sequence of prior tasks composes cleanly.

2. N/A

3. Verify:

   ```
   cargo fmt --all --check                    → no diff
   cargo clippy --workspace --all-targets -- -D warnings  → zero warnings
   cargo nextest run --workspace              → all green
   ```

   If `cargo fmt --all --check` reports a diff, run `cargo fmt --all`, re-stage, and fold the
   formatting fix into a new commit rather than amending a prior one (per this repo's git
   conventions).

4. Run: `git branch --show-current`
   Verify output is `develop`. Stop immediately if not.
   Commit (only if `cargo fmt` produced a diff to stage): `git commit -m "style: cargo fmt after flow tab implementation"`
