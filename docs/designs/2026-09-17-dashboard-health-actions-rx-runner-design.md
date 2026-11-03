# Design: Dashboard Health Actions with rx-runner

## Goal

Let taskit's dashboard run standard and coverage health-baseline updates from an Actions modal
without blocking the TUI, using a shared process runner also adopted by xtui.

## Approved Approach

Create and publish a runtime-neutral `rx-runner` crate in the rx workspace, migrate xtui's
process wrapper to it, then use it in taskit's synchronous dashboard action controller. Version
0.1.1 supersedes the initial 0.1.0 release with bounded terminal reporting when descendants retain
the direct child's output pipes.

## Context Map

### rx Files

| File                                            | Purpose                 | Change                                                                    |
| ----------------------------------------------- | ----------------------- | ------------------------------------------------------------------------- |
| `/Users/joe/dev/rx/Cargo.toml`                  | Workspace membership    | Add `crates/rx-runner`.                                                   |
| `/Users/joe/dev/rx/crates/rx-runner/Cargo.toml` | Publishable package     | Define the dependency-free `rx-runner` crate.                             |
| `/Users/joe/dev/rx/crates/rx-runner/src/lib.rs` | Shared process adapter  | Add bounded output capture, polling, cancellation, and kill/reap-on-drop. |
| `/Users/joe/dev/rx/crates/rx-runner/README.md`  | Crates.io documentation | Document lifecycle guarantees and a polling example.                      |
| `/Users/joe/dev/rx/Cargo.lock`                  | Workspace resolution    | Regenerate after adding the crate.                                        |

### xtui Files

| File                                | Purpose                | Change                                                                 |
| ----------------------------------- | ---------------------- | ---------------------------------------------------------------------- |
| `/Users/joe/dev/xtui/Cargo.toml`    | Dependencies           | Add published `rx-runner`.                                             |
| `/Users/joe/dev/xtui/src/runner.rs` | Source-command adapter | Preserve xtui's API while delegating process lifecycle to `rx-runner`. |
| `/Users/joe/dev/xtui/Cargo.lock`    | Dependency resolution  | Regenerate with `rx-runner`.                                           |

### taskit Files

| File                              | Purpose                     | Change                                                                                      |
| --------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------- |
| `Cargo.toml`                      | Workspace dependencies      | Add published `rx-runner`.                                                                  |
| `crates/taskit-tui/Cargo.toml`    | TUI dependencies            | Consume `rx-runner` from the workspace.                                                     |
| `crates/taskit-tui/src/action.rs` | Dashboard action controller | Define health actions, modal state, process polling, output tail, cancellation, and status. |
| `crates/taskit-tui/src/lib.rs`    | Event loop and key routing  | Poll actions and route modal keys before normal navigation.                                 |
| `crates/taskit-tui/src/app.rs`    | Dashboard state             | Own the action controller initialized from project executable/root metadata.                |
| `crates/taskit-tui/src/ui.rs`     | Rendering                   | Draw the Actions modal and action status in Activity/footer.                                |
| `Cargo.lock`                      | Dependency resolution       | Regenerate with `rx-runner`.                                                                |

### Existing References

- `/Users/joe/dev/xtui/src/runner.rs` already demonstrates piped stdout/stderr, polling, and
  kill-on-drop, but is coupled to Tokio.
- `/Users/joe/dev/rx/crates/rx-core/src/fan.rs` owns blocking fan-out execution and is not safe for
  TUI streaming because timeout polling does not drain child pipes.
- `crates/taskit-engine/src/health.rs::run` is already exposed as
  `taskit health check --update [--with-coverage]`; dashboard actions invoke this stable CLI seam.
- `crates/taskit-tui/src/lib.rs` has a synchronous 500 ms event loop suitable for polling a
  runtime-neutral child handle.

### Risk

- `rx-runner` is a new public crate and must pass package/API checks before irreversible publish.
- rx, xtui, and taskit all have pre-existing dirty worktrees; unrelated changes must not be staged,
  reverted, or included in any release commit.
- Publishing requires explicit handling of Cargo's dirty-worktree gate without hiding unrelated
  source state.
- The dashboard must bound retained output and reap children on cancel, quit, and drop.

## Crate Ownership

- **`rx-runner`** in `/Users/joe/dev/rx/crates/rx-runner` owns generic OS process lifecycle only.
- **xtui `runner`** remains the adapter from `SourceCommand` to shared `CommandSpec`.
- **`taskit-tui`** owns dashboard-specific action definitions, modal state, and presentation.
- `taskit-engine` remains unchanged; the child process re-enters taskit's existing CLI composition
  root instead of coupling the TUI to health internals.

## Public API: rx-runner

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec { /* private fields */ }

impl CommandSpec {
    pub fn new(program: impl Into<std::ffi::OsString>) -> Self;
    pub fn arg(self, arg: impl Into<std::ffi::OsString>) -> Self;
    pub fn args<I, S>(self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>;
    pub fn current_dir(self, path: impl Into<std::path::PathBuf>) -> Self;
    pub fn output_capacity(self, lines: usize) -> Self;
    pub fn program(&self) -> &std::ffi::OsStr;
    pub fn arguments(&self) -> impl Iterator<Item = &std::ffi::OsStr>;
    pub fn working_directory(&self) -> Option<&std::path::Path>;
    pub fn spawn(&self) -> std::io::Result<RunningProcess>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub stream: OutputStream,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessExit {
    pub code: Option<i32>,
    pub success: bool,
    pub elapsed: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessUpdate {
    pub lines: Vec<OutputLine>,
    pub dropped_lines: usize,
    pub exit: Option<ProcessExit>,
}

pub struct RunningProcess { /* private child, readers, and bounded buffer */ }

impl RunningProcess {
    pub fn id(&self) -> u32;
    pub fn poll(&mut self) -> std::io::Result<ProcessUpdate>;
    pub fn cancel(&mut self) -> std::io::Result<ProcessExit>;
}
```

`RunningProcess` implements a non-empty manual `Debug`. Its `Drop` implementation kills and reaps
an unfinished child, joins readers that have completed, and detaches any reader still held open by
a descendant process rather than blocking the caller. Reader threads write to a shared bounded
`VecDeque`; when full, the oldest line is dropped and reported by the next `ProcessUpdate`.

## taskit Action Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardAction {
    UpdateHealth,
    UpdateHealthWithCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionStatus {
    Idle,
    Running { action: DashboardAction },
    Succeeded { action: DashboardAction, elapsed: std::time::Duration },
    Failed {
        action: DashboardAction,
        elapsed: std::time::Duration,
        detail: String,
    },
    Cancelled { action: DashboardAction, elapsed: std::time::Duration },
}

pub(crate) struct ActionController { /* private menu/process/output state */ }

impl ActionController {
    pub(crate) fn new(binary: Option<String>, root: String) -> Self;
    pub(crate) fn open(&mut self);
    pub(crate) fn close(&mut self);
    pub(crate) fn select_next(&mut self);
    pub(crate) fn select_previous(&mut self);
    pub(crate) fn start_selected(&mut self);
    pub(crate) fn cancel(&mut self);
    pub(crate) fn poll(&mut self);
    pub(crate) fn is_open(&self) -> bool;
    pub(crate) fn selected(&self) -> usize;
    pub(crate) fn status(&self) -> &ActionStatus;
    pub(crate) fn output_tail(&self) -> &[String];
}
```

The two command specifications are fixed enum mappings, never user-provided shell strings:

- `taskit health check --update`
- `taskit health check --update --with-coverage`

## Interaction

- `a` opens or closes the Actions modal.
- `j`/`k` or arrow keys move selection.
- `Enter` starts the selected action and closes the modal.
- `x` cancels the running action while the modal is open.
- `Esc` closes the modal without changing the running action.
- While an action runs, the Activity panel shows action name, elapsed time, and latest output line.
- On completion, the next dashboard tick reloads `.health-baseline.json` through the existing
  `Snapshot::collect` path.

## xtui Compatibility

`xtui::runner::RunningTask` remains public with its existing methods. It becomes a thin wrapper
around `rx_runner::RunningProcess`, maps shared `OutputLine` values back to xtui's `Vec<String>`,
and maps signal-only exits to `-1` so xtui always observes terminal completion. Existing async
function signatures remain unchanged to avoid an xtui public API break.

## Data Flow

1. Modal selection -> fixed `DashboardAction` -> `rx_runner::CommandSpec`.
2. `CommandSpec::spawn` -> child process + bounded shared output buffer.
3. TUI tick -> `ActionController::poll` -> output/status update -> Activity/modal rendering.
4. Successful child writes `.health-baseline.json` -> next `Snapshot::collect` -> refreshed Health
   panel.
5. xtui source adapter -> same `CommandSpec`/`RunningProcess` -> existing xtui output and history.

## Hexagonal Boundaries

- `rx-runner` is the reusable process adapter and exposes typed process lifecycle values.
- taskit's `DashboardAction` and xtui's `SourceCommand` are application-level command intents.
- Consumers map intent to `CommandSpec`; neither UI renderer constructs shell commands.
- No shell interpolation is used; program and arguments remain separate `OsString` values.

## Testing

- `rx-runner`: command builder, stdout/stderr capture, bounded overflow reporting, terminal exit,
  cancellation, and drop-reap behavior on Unix and Windows fixture commands.
- xtui: retain runner output-capture tests and add signal-exit mapping coverage where practical.
- taskit-tui: action selection/wrapping, exact health command arguments, spawn failure, status
  transitions, cancellation, modal rendering, footer hint, and refreshed Health panel smoke test.

## Release

1. Run rx workspace fmt, check, Clippy, tests, package, and API checklist.
2. Publish `rx-runner` version `0.1.1` to crates.io and verify availability.
3. Add registry dependency `rx-runner = "0.1.1"` to xtui and taskit; do not retain sibling paths.
4. Run each consumer's required gates and interactive dashboard/xtui smoke tests.

## Out of Scope

- Migrating `rx_core::fan::fan_out` in this change.
- Arbitrary user-entered commands in taskit's Actions modal.
- Persisting action history beyond the current dashboard session.
- More dashboard actions beyond the two health updates.
- Changing taskit health collection or baseline semantics.

## Risk Summary

- [ ] Breaking API changes: no for xtui/taskit; `rx-runner` is new at 0.1.x.
- [ ] New dependencies: xtui and taskit add published `rx-runner`; `rx-runner` has no dependencies.
- [ ] Feature flag required: no.
- [ ] Irreversible operation: crates.io publish requires explicit approved execution after all gates.
