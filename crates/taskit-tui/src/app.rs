//! Interactive dashboard state: active tab and per-tab scroll position.
//!
//! Kept separate from [`crate::Snapshot`] because it's session state (what
//! the user is looking at), not point-in-time data pulled from disk.

use std::fs;
use std::path::Path;

use taskit_engine::ctx::Ctx;
use taskit_types::step::PipelineRunContext;

use crate::action::ActionController;
use crate::snapshot::Snapshot;

const INSTRUCTION_FILE_CANDIDATES: &[&str] =
    &["AGENTS.md", "CLAUDE.md", ".github/copilot-instructions.md"];
const AGENT_CONFIG_CANDIDATES: &[&str] = &[
    "taskit.toml",
    "Cruxfile",
    ".ctx/godmode/tasks.yaml",
    ".ctx/opavs/tasks.yaml",
];
const AGENT_TASK_FILE_CANDIDATES: &[&str] = &[".ctx/godmode/tasks.yaml", ".ctx/opavs/tasks.yaml"];
const AGENT_PLAN_DIR_CANDIDATES: &[&str] = &[".ctx/godmode/plans", ".ctx/opavs/plans"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectInfo {
    pub(crate) name: String,
    pub(crate) root: String,
    pub(crate) taskit_version: String,
    pub(crate) taskit_binary: Option<String>,
    pub(crate) git_sha: Option<String>,
    pub(crate) rustc_version: Option<String>,
    pub(crate) cargo_version: Option<String>,
    pub(crate) os: &'static str,
    pub(crate) arch: &'static str,
    pub(crate) logical_cpus: usize,
    pub(crate) ci_step_names: Vec<String>,
    pub(crate) ci_gate_count: usize,
    pub(crate) protocol_surface_count: usize,
    pub(crate) agentic: AgenticContext,
}

impl Default for ProjectInfo {
    fn default() -> Self {
        Self {
            name: "workspace".to_string(),
            root: String::new(),
            taskit_version: String::new(),
            taskit_binary: None,
            git_sha: None,
            rustc_version: None,
            cargo_version: None,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            logical_cpus: 1,
            ci_step_names: Vec::new(),
            ci_gate_count: 0,
            protocol_surface_count: 0,
            agentic: AgenticContext::default(),
        }
    }
}

impl ProjectInfo {
    fn collect(ctx: &Ctx, run_context: &PipelineRunContext) -> Self {
        let name = ctx
            .root()
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("workspace")
            .to_string();
        let ci_step_names = ctx
            .ci()
            .map(|ci| ci.steps.iter().map(|step| step.name.clone()).collect())
            .unwrap_or_default();
        let ci_gate_count = ctx
            .ci()
            .map(|ci| ci.steps.iter().filter(|step| step.gate).count())
            .unwrap_or_default();
        let protocol_surface_count = ctx
            .proto()
            .map(|protocol| protocol.surfaces.len())
            .unwrap_or_default();

        Self {
            name,
            root: run_context.workspace_root.clone(),
            taskit_version: run_context.taskit_version.clone(),
            taskit_binary: run_context.taskit_binary.clone(),
            git_sha: run_context.git_sha.clone(),
            rustc_version: run_context.rustc_version.clone(),
            cargo_version: run_context.cargo_version.clone(),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            logical_cpus: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
            ci_step_names,
            ci_gate_count,
            protocol_surface_count,
            agentic: AgenticContext::collect(ctx.root()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgenticContext {
    pub(crate) instruction_files: Vec<String>,
    pub(crate) config_files: Vec<String>,
    pub(crate) plan_count: usize,
    pub(crate) tasks: Option<AgentTaskCounts>,
}

impl AgenticContext {
    fn collect(root: &Path) -> Self {
        let instruction_files = existing_files(root, INSTRUCTION_FILE_CANDIDATES);
        let config_files = existing_files(root, AGENT_CONFIG_CANDIDATES);
        let plan_count = AGENT_PLAN_DIR_CANDIDATES
            .iter()
            .map(|path| markdown_file_count(&root.join(path)))
            .sum();

        let task_counts: Vec<AgentTaskCounts> = AGENT_TASK_FILE_CANDIDATES
            .iter()
            .filter_map(|path| fs::read_to_string(root.join(path)).ok())
            .map(|contents| AgentTaskCounts::parse(&contents))
            .collect();
        let tasks = (!task_counts.is_empty()).then(|| {
            task_counts
                .into_iter()
                .fold(AgentTaskCounts::default(), AgentTaskCounts::merge)
        });

        Self {
            instruction_files,
            config_files,
            plan_count,
            tasks,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AgentTaskCounts {
    pub(crate) pending: usize,
    pub(crate) active: usize,
    pub(crate) blocked: usize,
    pub(crate) done: usize,
}

impl AgentTaskCounts {
    fn parse(contents: &str) -> Self {
        let mut counts = Self::default();
        for status in contents.lines().filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("- ").unwrap_or(line);
            line.strip_prefix("status:")
                .map(|value| value.trim().trim_matches(['\'', '"']))
        }) {
            match status {
                "pending" => counts.pending += 1,
                "active" | "running" | "in_progress" | "in-progress" => counts.active += 1,
                "blocked" => counts.blocked += 1,
                "done" | "completed" => counts.done += 1,
                _ => {}
            }
        }
        counts
    }

    fn merge(mut self, other: Self) -> Self {
        self.pending += other.pending;
        self.active += other.active;
        self.blocked += other.blocked;
        self.done += other.done;
        self
    }
}

fn existing_files(root: &Path, candidates: &[&str]) -> Vec<String> {
    candidates
        .iter()
        .filter(|path| root.join(path).is_file())
        .map(|path| (*path).to_string())
        .collect()
}

fn markdown_file_count(path: &Path) -> usize {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Top-level dashboard tabs.
pub enum Tab {
    /// Summary metrics and quick status cards.
    Overview,
    /// Workspace crate inventory.
    Crates,
    /// Recent CI/telemetry history.
    History,
    /// Flow/branch promotion state.
    Flow,
}

impl Tab {
    /// Ordered tab list used for keyboard cycling.
    pub const ALL: [Tab; 4] = [Tab::Overview, Tab::Crates, Tab::History, Tab::Flow];

    /// Human-friendly tab title.
    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Crates => "Crates",
            Tab::History => "History",
            Tab::Flow => "Flow",
        }
    }
}

/// Mutable UI session state for the dashboard runtime.
pub struct App {
    /// Currently active top-level tab.
    pub active_tab: Tab,
    /// Workspace member crate names, fetched once at startup via `cargo
    /// metadata` — cheap enough to shell out for once, too slow to refetch
    /// on every 500ms tick.
    pub crate_names: Vec<String>,
    pub(crate) project_info: ProjectInfo,
    pub(crate) actions: ActionController,
    /// Scroll offset for the active tab content.
    pub scroll: u16,
}

impl App {
    /// Create a new dashboard app state from the runtime context.
    pub fn new(ctx: &Ctx) -> Self {
        let run_context = ctx.pipeline_run_context();
        let project_info = ProjectInfo::collect(ctx, &run_context);
        let actions = ActionController::new(
            project_info.taskit_binary.clone(),
            project_info.root.clone(),
        );
        Self {
            active_tab: Tab::Overview,
            project_info,
            actions,
            crate_names: run_context.workspace_members,
            scroll: 0,
        }
    }

    /// Advance to the next tab, wrapping at the end.
    pub fn next_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
        self.scroll = 0;
    }

    /// Move to the previous tab, wrapping at the beginning.
    pub fn prev_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
        self.scroll = 0;
    }

    /// Scroll down by one row.
    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    /// Scroll up by one row.
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scroll down by one page increment.
    pub fn scroll_down_page(&mut self) {
        self.scroll = self.scroll.saturating_add(10);
    }

    /// Scroll up by one page increment.
    pub fn scroll_up_page(&mut self) {
        self.scroll = self.scroll.saturating_sub(10);
    }

    /// Jump to the top of the current tab content.
    pub fn scroll_top(&mut self) {
        self.scroll = 0;
    }

    /// Jump toward the bottom of the current tab content.
    pub fn scroll_bottom(&mut self) {
        self.scroll = u16::MAX;
    }

    /// Number of scrollable rows in the active tab for the given snapshot —
    /// used to clamp `scroll` so it never runs past the content.
    ///
    /// Clamped (not cast) to `u16::MAX` — a plain `as u16` would silently
    /// wrap for row counts above 65535 instead of saturating.
    fn max_scroll(&self, snapshot: &Snapshot) -> u16 {
        let rows = match self.active_tab {
            Tab::Overview => 0,
            Tab::Crates => self.crate_names.len(),
            Tab::History => snapshot.records.len(),
            Tab::Flow => 0,
        };
        u16::try_from(rows.saturating_sub(1)).unwrap_or(u16::MAX)
    }

    /// Clamp scroll offset to the active tab's available row range.
    pub fn clamp_scroll(&mut self, snapshot: &Snapshot) {
        self.scroll = self.scroll.min(self.max_scroll(snapshot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(active_tab: Tab, crate_count: usize) -> App {
        App {
            active_tab,
            crate_names: (0..crate_count).map(|i| format!("crate-{i}")).collect(),
            project_info: ProjectInfo::default(),
            actions: ActionController::default(),
            scroll: 0,
        }
    }

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

    #[test]
    fn switching_tabs_resets_scroll() {
        let mut app = app(Tab::Overview, 0);
        app.scroll = 7;
        app.next_tab();
        assert_eq!(app.scroll, 0);

        app.scroll = 7;
        app.prev_tab();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn scroll_step_and_page_move_by_expected_amounts() {
        let mut app = app(Tab::Crates, 100);
        app.scroll_down();
        assert_eq!(app.scroll, 1);
        app.scroll_down_page();
        assert_eq!(app.scroll, 11);
        app.scroll_up();
        assert_eq!(app.scroll, 10);
        app.scroll_up_page();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut app = app(Tab::Crates, 100);
        app.scroll_up();
        assert_eq!(app.scroll, 0);
        app.scroll_up_page();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn scroll_top_and_bottom_clamp_to_content() {
        let mut app = app(Tab::Crates, 5);
        let snapshot = snapshot_with_records(0);

        app.scroll_bottom();
        app.clamp_scroll(&snapshot);
        assert_eq!(app.scroll, 4, "5 crates -> max index 4");

        app.scroll_top();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn clamp_scroll_uses_records_for_history_tab() {
        let mut app = app(Tab::History, 999); // crate_names must be ignored here
        let snapshot = snapshot_with_records(3);

        app.scroll_bottom();
        app.clamp_scroll(&snapshot);
        assert_eq!(app.scroll, 2, "3 records -> max index 2");
    }

    #[test]
    fn clamp_scroll_overview_tab_has_no_scrollable_content() {
        let mut app = app(Tab::Overview, 50);
        let snapshot = snapshot_with_records(50);

        app.scroll_bottom();
        app.clamp_scroll(&snapshot);
        assert_eq!(app.scroll, 0, "Overview has no scrollable rows");
    }

    #[test]
    fn clamp_scroll_flow_tab_has_no_scrollable_content() {
        let mut app = app(Tab::Flow, 50);
        let snapshot = snapshot_with_records(50);

        app.scroll_bottom();
        app.clamp_scroll(&snapshot);
        assert_eq!(app.scroll, 0, "Flow has no scrollable rows");
    }

    #[test]
    fn max_scroll_saturates_instead_of_wrapping_past_u16_max() {
        // Regression: `rows.saturating_sub(1) as u16` would wrap 69999 down
        // to 4463 instead of saturating at u16::MAX.
        let mut app = app(Tab::Crates, 70_000);
        app.scroll_bottom();
        app.clamp_scroll(&snapshot_with_records(0));
        assert_eq!(app.scroll, u16::MAX);
    }

    #[test]
    fn agent_task_counts_parse_common_statuses() {
        let counts = AgentTaskCounts::parse(
            "status: pending\nstatus: active\nstatus: running\nstatus: in-progress\n\
             status: blocked\nstatus: done\nstatus: completed\nstatus: unknown\n",
        );

        assert_eq!(counts.pending, 1);
        assert_eq!(counts.active, 3);
        assert_eq!(counts.blocked, 1);
        assert_eq!(counts.done, 2);
    }

    #[test]
    fn agentic_context_discovers_common_project_files() {
        let temp = tempfile::tempdir().expect("create temp project");
        fs::create_dir_all(temp.path().join(".ctx/godmode/plans")).expect("create plan directory");
        fs::write(temp.path().join("AGENTS.md"), "# Instructions").expect("write instructions");
        fs::write(temp.path().join("taskit.toml"), "[workspace]").expect("write taskit config");
        fs::write(
            temp.path().join(".ctx/godmode/tasks.yaml"),
            "tasks:\n- status: pending\n- status: done\n",
        )
        .expect("write task graph");
        fs::write(temp.path().join(".ctx/godmode/plans/overview.md"), "# Plan")
            .expect("write plan");

        let context = AgenticContext::collect(temp.path());

        assert_eq!(context.instruction_files, vec!["AGENTS.md"]);
        assert_eq!(
            context.config_files,
            vec!["taskit.toml", ".ctx/godmode/tasks.yaml"]
        );
        assert_eq!(context.plan_count, 1);
        assert_eq!(
            context.tasks,
            Some(AgentTaskCounts {
                pending: 1,
                done: 1,
                ..AgentTaskCounts::default()
            })
        );
    }

    use proptest::{prop_assert, prop_assert_eq, proptest};

    proptest! {
        #[test]
        fn clamp_scroll_never_exceeds_row_count_minus_one(
            crate_count in 0usize..500,
            requested_scroll in 0u16..=u16::MAX,
        ) {
            let mut app = app(Tab::Crates, crate_count);
            app.scroll = requested_scroll;
            app.clamp_scroll(&snapshot_with_records(0));

            let expected_max = crate_count.saturating_sub(1) as u16;
            prop_assert!(app.scroll <= expected_max);
            // Never clamps below what was requested if the request already fit.
            if requested_scroll <= expected_max {
                prop_assert_eq!(app.scroll, requested_scroll);
            }
        }

        #[test]
        fn tab_cycling_always_lands_on_a_valid_tab(steps in 0usize..20) {
            let mut app = app(Tab::Overview, 0);
            for i in 0..steps {
                if i % 2 == 0 {
                    app.next_tab();
                } else {
                    app.prev_tab();
                }
            }
            prop_assert!(Tab::ALL.contains(&app.active_tab));
        }
    }
}
