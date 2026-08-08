//! Interactive dashboard state: active tab and per-tab scroll position.
//!
//! Kept separate from [`crate::Snapshot`] because it's session state (what
//! the user is looking at), not point-in-time data pulled from disk.

use taskit_engine::ctx::Ctx;

use crate::snapshot::Snapshot;

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

pub struct App {
    pub active_tab: Tab,
    /// Workspace member crate names, fetched once at startup via `cargo
    /// metadata` — cheap enough to shell out for once, too slow to refetch
    /// on every 500ms tick.
    pub crate_names: Vec<String>,
    pub scroll: u16,
}

impl App {
    pub fn new(ctx: &Ctx) -> Self {
        Self {
            active_tab: Tab::Overview,
            crate_names: ctx.pipeline_run_context().workspace_members,
            scroll: 0,
        }
    }

    pub fn next_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
        self.scroll = 0;
    }

    pub fn prev_tab(&mut self) {
        let idx = Tab::ALL
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        self.active_tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
        self.scroll = 0;
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down_page(&mut self) {
        self.scroll = self.scroll.saturating_add(10);
    }

    pub fn scroll_up_page(&mut self) {
        self.scroll = self.scroll.saturating_sub(10);
    }

    pub fn scroll_top(&mut self) {
        self.scroll = 0;
    }

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
