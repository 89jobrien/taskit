//! Non-blocking dashboard actions backed by `rx-runner`.

use std::time::{Duration, Instant};

use rx_runner::{CommandSpec, OutputStream, RunningProcess};

const OUTPUT_TAIL_LINES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardAction {
    UpdateHealth,
    UpdateHealthWithCoverage,
}

impl DashboardAction {
    pub(crate) const ALL: [Self; 2] = [Self::UpdateHealth, Self::UpdateHealthWithCoverage];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::UpdateHealth => "Update health baseline",
            Self::UpdateHealthWithCoverage => "Update health + coverage",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::UpdateHealth => "Run tests, Clippy, safety, and project metrics",
            Self::UpdateHealthWithCoverage => "Also run slower workspace coverage instrumentation",
        }
    }

    fn command_spec(self, binary: Option<&str>, root: &str) -> Result<CommandSpec, String> {
        let binary = binary.ok_or_else(|| "taskit executable is unavailable".to_string())?;
        let mut spec = CommandSpec::new(binary)
            .args(["health", "check", "--update"])
            .current_dir(root);
        if matches!(self, Self::UpdateHealthWithCoverage) {
            spec = spec.arg("--with-coverage");
        }
        Ok(spec)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionStatus {
    Idle,
    Running {
        action: DashboardAction,
    },
    Succeeded {
        action: DashboardAction,
        elapsed: Duration,
    },
    Failed {
        action: DashboardAction,
        elapsed: Duration,
        detail: String,
    },
    Cancelled {
        action: DashboardAction,
        elapsed: Duration,
    },
}

struct RunningAction {
    action: DashboardAction,
    process: RunningProcess,
    started_at: Instant,
}

pub(crate) struct ActionController {
    binary: Option<String>,
    root: String,
    menu_open: bool,
    selected: usize,
    running: Option<RunningAction>,
    status: ActionStatus,
    output_tail: Vec<String>,
}

impl ActionController {
    pub(crate) fn new(binary: Option<String>, root: String) -> Self {
        Self {
            binary,
            root,
            menu_open: false,
            selected: 0,
            running: None,
            status: ActionStatus::Idle,
            output_tail: Vec::with_capacity(OUTPUT_TAIL_LINES),
        }
    }

    pub(crate) fn open(&mut self) {
        self.menu_open = true;
    }

    pub(crate) fn close(&mut self) {
        self.menu_open = false;
    }

    pub(crate) fn toggle(&mut self) {
        self.menu_open = !self.menu_open;
    }

    pub(crate) fn select_next(&mut self) {
        self.selected = (self.selected + 1) % DashboardAction::ALL.len();
    }

    pub(crate) fn select_previous(&mut self) {
        self.selected =
            (self.selected + DashboardAction::ALL.len() - 1) % DashboardAction::ALL.len();
    }

    pub(crate) fn start_selected(&mut self) {
        if self.running.is_some() {
            return;
        }
        let action = DashboardAction::ALL[self.selected];
        self.output_tail.clear();
        self.menu_open = false;
        let spec = match action.command_spec(self.binary.as_deref(), &self.root) {
            Ok(spec) => spec,
            Err(detail) => {
                self.status = ActionStatus::Failed {
                    action,
                    elapsed: Duration::ZERO,
                    detail,
                };
                return;
            }
        };
        match spec.spawn() {
            Ok(process) => {
                self.running = Some(RunningAction {
                    action,
                    process,
                    started_at: Instant::now(),
                });
                self.status = ActionStatus::Running { action };
            }
            Err(error) => {
                self.status = ActionStatus::Failed {
                    action,
                    elapsed: Duration::ZERO,
                    detail: error.to_string(),
                };
            }
        }
    }

    pub(crate) fn cancel(&mut self) {
        let Some(mut running) = self.running.take() else {
            return;
        };
        let elapsed = running.started_at.elapsed();
        self.status = match running.process.cancel() {
            Ok(_) => ActionStatus::Cancelled {
                action: running.action,
                elapsed,
            },
            Err(error) => ActionStatus::Failed {
                action: running.action,
                elapsed,
                detail: error.to_string(),
            },
        };
    }

    pub(crate) fn poll(&mut self) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        let action = running.action;
        let elapsed = running.started_at.elapsed();
        let update = match running.process.poll() {
            Ok(update) => update,
            Err(error) => {
                self.status = ActionStatus::Failed {
                    action,
                    elapsed,
                    detail: error.to_string(),
                };
                self.running = None;
                return;
            }
        };

        for line in update.lines {
            let prefix = match line.stream {
                OutputStream::Stdout => "",
                OutputStream::Stderr => "! ",
            };
            self.push_output(format!("{prefix}{}", line.text));
        }
        if update.dropped_lines > 0 {
            self.push_output(format!("{} earlier lines dropped", update.dropped_lines));
        }
        if let Some(exit) = update.exit {
            self.status = if exit.success {
                ActionStatus::Succeeded {
                    action,
                    elapsed: exit.elapsed,
                }
            } else {
                ActionStatus::Failed {
                    action,
                    elapsed: exit.elapsed,
                    detail: self
                        .output_tail
                        .last()
                        .cloned()
                        .unwrap_or_else(|| format!("exit code {:?}", exit.code)),
                }
            };
            self.running = None;
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.menu_open
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn status(&self) -> &ActionStatus {
        &self.status
    }

    pub(crate) fn elapsed(&self) -> Option<Duration> {
        self.running
            .as_ref()
            .map(|running| running.started_at.elapsed())
    }

    pub(crate) fn output_tail(&self) -> &[String] {
        &self.output_tail
    }

    fn push_output(&mut self, line: String) {
        if self.output_tail.len() == OUTPUT_TAIL_LINES {
            self.output_tail.remove(0);
        }
        self.output_tail.push(line);
    }
}

impl Default for ActionController {
    fn default() -> Self {
        Self::new(None, String::new())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn health_actions_use_expected_arguments() {
        let standard = DashboardAction::UpdateHealth
            .command_spec(Some("taskit"), "/workspace")
            .expect("standard health spec");
        let coverage = DashboardAction::UpdateHealthWithCoverage
            .command_spec(Some("taskit"), "/workspace")
            .expect("coverage health spec");

        assert_eq!(standard.program(), OsStr::new("taskit"));
        assert_eq!(
            standard.arguments().collect::<Vec<_>>(),
            vec![
                OsStr::new("health"),
                OsStr::new("check"),
                OsStr::new("--update")
            ]
        );
        assert_eq!(
            coverage.arguments().collect::<Vec<_>>(),
            vec![
                OsStr::new("health"),
                OsStr::new("check"),
                OsStr::new("--update"),
                OsStr::new("--with-coverage")
            ]
        );
    }

    #[test]
    fn action_selection_wraps_in_both_directions() {
        let mut controller = ActionController::default();
        controller.select_previous();
        assert_eq!(controller.selected(), 1);
        controller.select_next();
        assert_eq!(controller.selected(), 0);
    }

    #[test]
    fn missing_binary_becomes_failed_status() {
        let mut controller = ActionController::default();

        controller.start_selected();

        assert!(matches!(controller.status(), ActionStatus::Failed { .. }));
        assert!(!controller.is_open());
    }

    #[cfg(unix)]
    #[test]
    fn completed_process_updates_action_status() {
        let mut controller = ActionController::new(Some("true".to_string()), ".".to_string());
        controller.start_selected();

        for _ in 0..100 {
            controller.poll();
            if !controller.is_running() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(matches!(
            controller.status(),
            ActionStatus::Succeeded {
                action: DashboardAction::UpdateHealth,
                ..
            }
        ));
    }
}
