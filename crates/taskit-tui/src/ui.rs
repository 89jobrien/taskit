//! Widget layout for the dashboard frame: a tab bar plus a per-tab body.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Sparkline, Tabs};

use taskit_types::config::ConflictResolverKind;

use crate::action::{ActionController, ActionStatus, DashboardAction};
use crate::app::{App, Tab};
use crate::snapshot::Snapshot;

/// Renders the dashboard tabs, active-tab body, footer, and open actions modal.
pub fn render(frame: &mut Frame, app: &App, snapshot: &Snapshot) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_tabs(frame, chunks[0], app);

    match app.active_tab {
        Tab::Overview => render_overview(frame, chunks[1], app, snapshot),
        Tab::Crates => render_crates(frame, chunks[1], app),
        Tab::History => render_history(frame, chunks[1], app, snapshot),
        Tab::Flow => render_flow(frame, chunks[1], snapshot),
    }

    render_footer(frame, chunks[2], app, snapshot);
    if app.actions.is_open() {
        render_actions_modal(frame, frame.area(), &app.actions);
    }
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.title())).collect();
    let selected = Tab::ALL
        .iter()
        .position(|&t| t == app.active_tab)
        .unwrap_or(0);
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("taskit — real-time governance dashboard"),
        )
        .select(selected)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );
    frame.render_widget(tabs, area);
}

fn render_overview(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    render_project_header(frame, rows[0], app, snapshot);

    let middle = overview_columns(rows[1]);
    render_workspace_pipeline(frame, middle[0], app);
    render_agentic_context(frame, middle[1], app);

    let bottom = overview_columns(rows[2]);
    render_health(frame, bottom[0], snapshot);
    render_activity(frame, bottom[1], app, snapshot);
}

fn overview_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area)
}

fn render_project_header(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let block = Block::default()
        .title(" Project / System ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(0)])
        .split(inner);
    let logo_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let logo = vec![
        Line::from(Span::styled("  __             __   _ __", logo_style)),
        Line::from(Span::styled(" / /_____ ______/ /__(_) /_", logo_style)),
        Line::from(Span::styled("/ __/ __ `/ ___/ //_/ / __/", logo_style)),
        Line::from(Span::styled("/ /_/ /_/ (__  ) ,< / / /_", logo_style)),
        Line::from(Span::styled("\\__/\\__,_/____/_/|_/_/\\__/", logo_style)),
        Line::from(Span::styled(
            "    project command center",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(logo), columns[0]);

    let project = &app.project_info;
    let branch = snapshot
        .flow_status
        .as_ref()
        .map(|status| status.current_branch.as_str())
        .unwrap_or("unknown");
    let sha = project
        .git_sha
        .as_deref()
        .map(short_sha)
        .unwrap_or_else(|| "unknown".to_string());
    let rustc = project.rustc_version.as_deref().unwrap_or("unavailable");
    let cargo = project.cargo_version.as_deref().unwrap_or("unavailable");
    let lines = vec![
        Line::from(Span::styled(
            project.name.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        key_value("Root", project.root.clone()),
        key_value("Git", format!("{branch} @ {sha}")),
        key_value(
            "Host",
            format!(
                "{}/{} - {} logical CPUs",
                project.os, project.arch, project.logical_cpus
            ),
        ),
        key_value("taskit", format!("v{}", project.taskit_version)),
        key_value("Rust", rustc.to_string()),
        key_value("Cargo", cargo.to_string()),
    ];
    frame.render_widget(Paragraph::new(lines), columns[1]);
}

fn render_workspace_pipeline(frame: &mut Frame, area: Rect, app: &App) {
    let project = &app.project_info;
    let ci_summary = if project.ci_step_names.is_empty() {
        "no configured steps".to_string()
    } else {
        let gate_label = if project.ci_gate_count == 1 {
            "gate"
        } else {
            "gates"
        };
        format!(
            "{} steps, {} {gate_label}",
            project.ci_step_names.len(),
            project.ci_gate_count
        )
    };
    let lines = vec![
        key_value("Crates", app.crate_names.len().to_string()),
        key_value("Members", preview(&app.crate_names, 6)),
        key_value("CI", ci_summary),
        key_value("Steps", preview(&project.ci_step_names, 5)),
        key_value(
            "Protocol",
            format!("{} surfaces", project.protocol_surface_count),
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Workspace & Pipeline ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_agentic_context(frame: &mut Frame, area: Rect, app: &App) {
    let agentic = &app.project_info.agentic;
    let task_line = agentic.tasks.map_or_else(
        || "no task graph".to_string(),
        |tasks| {
            format!(
                "{} pending - {} active - {} blocked - {} done",
                tasks.pending, tasks.active, tasks.blocked, tasks.done
            )
        },
    );
    let lines = vec![
        key_value("Instructions", preview(&agentic.instruction_files, 3)),
        key_value("Config", preview(&agentic.config_files, 4)),
        key_value("Plans", agentic.plan_count.to_string()),
        key_value("Tasks", task_line),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Agentic Context ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_health(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default().title(" Health ").borders(Borders::ALL);
    let Some(b) = &snapshot.baseline else {
        let lines = vec![
            Line::from("No health baseline found."),
            Line::from("Run `taskit health check --update`."),
            protocol_status_line(snapshot),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
        return;
    };

    let test_color = if b.tests.failed > 0 {
        Color::Red
    } else {
        Color::Green
    };
    let clippy_clean = b.clippy.warnings == 0 && b.clippy.errors == 0;
    let lines = vec![
        Line::from(vec![
            label("Tests"),
            Span::styled(
                format!(
                    "{}/{} passed, {} failed",
                    b.tests.passed, b.tests.total, b.tests.failed
                ),
                Style::default().fg(test_color),
            ),
        ]),
        Line::from(vec![
            label("Clippy"),
            Span::styled(
                format!("{} warnings, {} errors", b.clippy.warnings, b.clippy.errors),
                Style::default().fg(if clippy_clean {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
        ]),
        key_value("TODO/FIXME", b.todo_fixme.to_string()),
        key_value(
            "Version",
            format!("{} (consistent: {})", b.version, b.versions_consistent),
        ),
        key_value("Baseline", b.date.clone()),
        protocol_status_line(snapshot),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn protocol_status_line(snapshot: &Snapshot) -> Line<'static> {
    match &snapshot.protocol_drift {
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
    }
}

fn render_activity(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let ci_line = match snapshot.last_ci_passed {
        Some(true) => Line::from(vec![
            label("Latest CI"),
            Span::styled("PASS", Style::default().fg(Color::Green)),
        ]),
        Some(false) => Line::from(vec![
            label("Latest CI"),
            Span::styled("FAIL", Style::default().fg(Color::Red)),
        ]),
        None => key_value("Latest CI", "no data".to_string()),
    };
    let drift_line = match &snapshot.ci_duration_drift {
        Some(drift) if drift.regressed => Line::from(vec![
            label("Duration"),
            Span::styled(
                format!("DRIFT {:+.1}% vs mean", drift.delta_pct),
                Style::default().fg(Color::Red),
            ),
        ]),
        Some(drift) => Line::from(vec![
            label("Duration"),
            Span::styled(
                format!("stable {:+.1}% vs mean", drift.delta_pct),
                Style::default().fg(Color::Green),
            ),
        ]),
        None => key_value("Duration", "not enough history".to_string()),
    };
    let flow_line = snapshot.flow_state.as_ref().map_or_else(
        || key_value("Flow", "idle".to_string()),
        |state| {
            Line::from(vec![
                label("Flow"),
                Span::styled(
                    format!("resume {:?}", state.phase),
                    Style::default().fg(Color::Yellow),
                ),
            ])
        },
    );
    let mut lines = vec![
        ci_line,
        key_value("Runs (7d)", snapshot.ci_run_count.to_string()),
        drift_line,
        flow_line,
        key_value(
            "Flow auto",
            format!("{} samples", snapshot.flow_auto_duration_history.len()),
        ),
        action_status_line(&app.actions),
    ];
    if let Some(output) = app.actions.output_tail().last() {
        lines.push(key_value("Output", output.clone()));
    }
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title(" Activity ").borders(Borders::ALL)),
        area,
    );
}

fn key_value(label_text: &'static str, value: String) -> Line<'static> {
    Line::from(vec![label(label_text), Span::raw(value)])
}

fn label(text: &'static str) -> Span<'static> {
    Span::styled(
        format!("{text:<13}"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn preview(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return "none".to_string();
    }

    let mut summary = items
        .iter()
        .take(limit)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = items.len().saturating_sub(limit);
    if remaining > 0 {
        summary.push_str(&format!(" +{remaining}"));
    }
    summary
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

fn action_status_line(actions: &ActionController) -> Line<'static> {
    match actions.status() {
        ActionStatus::Idle => key_value("Action", "idle - press a".to_string()),
        ActionStatus::Running { action } => key_value(
            "Action",
            format!(
                "{} ({:.1}s)",
                action.title(),
                actions.elapsed().unwrap_or_default().as_secs_f32()
            ),
        ),
        ActionStatus::Succeeded { action, elapsed } => Line::from(vec![
            label("Action"),
            Span::styled(
                format!("{} passed ({:.1}s)", action.title(), elapsed.as_secs_f32()),
                Style::default().fg(Color::Green),
            ),
        ]),
        ActionStatus::Failed {
            action,
            elapsed,
            detail,
        } => Line::from(vec![
            label("Action"),
            Span::styled(
                format!(
                    "{} failed ({:.1}s): {detail}",
                    action.title(),
                    elapsed.as_secs_f32()
                ),
                Style::default().fg(Color::Red),
            ),
        ]),
        ActionStatus::Cancelled { action, elapsed } => Line::from(vec![
            label("Action"),
            Span::styled(
                format!(
                    "{} cancelled ({:.1}s)",
                    action.title(),
                    elapsed.as_secs_f32()
                ),
                Style::default().fg(Color::Yellow),
            ),
        ]),
    }
}

fn render_actions_modal(frame: &mut Frame, area: Rect, actions: &ActionController) {
    let area = centered_modal_area(area);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    for (index, action) in DashboardAction::ALL.iter().copied().enumerate() {
        let selected = index == actions.selected();
        let style = if selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(if selected { "> " } else { "  " }, style),
            Span::styled(action.title(), style),
        ]));
        lines.push(Line::from(Span::styled(
            format!("    {}", action.description()),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));
    lines.push(action_status_line(actions));
    if let Some(output) = actions.output_tail().last() {
        lines.push(key_value("Latest", output.clone()));
    }
    lines.push(Line::from(Span::styled(
        if actions.is_running() {
            "Enter unavailable while running - x cancel - Esc close"
        } else {
            "j/k select - Enter run - Esc close"
        },
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title(" Actions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn centered_modal_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(82);
    let height = area.height.saturating_sub(2).min(12);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn render_crates(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(format!("Workspace Crates ({})", app.crate_names.len()))
        .borders(Borders::ALL);
    if app.crate_names.is_empty() {
        frame.render_widget(
            Paragraph::new("No workspace members found (is `cargo metadata` available?).")
                .block(block),
            area,
        );
        return;
    }
    let lines: Vec<Line> = app
        .crate_names
        .iter()
        .enumerate()
        .map(|(i, name)| Line::from(format!("{:>3}  {name}", i + 1)))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((app.scroll, 0)),
        area,
    );
}

fn render_history(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let block = Block::default()
        .title(format!("CI History — {} runs (7d)", snapshot.records.len()))
        .borders(Borders::ALL);
    if snapshot.records.is_empty() {
        frame.render_widget(
            Paragraph::new("No CI runs recorded yet — run `taskit ci`.").block(block),
            area,
        );
        return;
    }
    let lines: Vec<Line> = snapshot
        .records
        .iter()
        .map(|r| {
            let duration = r
                .metrics
                .iter()
                .find(|m| m.name == "ci_duration_ms")
                .map(|m| format!("{:.0}ms", m.value))
                .unwrap_or_else(|| "-".to_string());
            let passed = r.metrics.iter().find(|m| m.name == "ci_passed");
            let sha = r.git_sha.as_deref().unwrap_or("-");
            match passed.map(|m| m.value >= 1.0) {
                Some(true) => Line::from(Span::styled(
                    format!("{:<25} PASS  {duration:>10}  {sha}", r.timestamp),
                    Style::default().fg(Color::Green),
                )),
                Some(false) => Line::from(Span::styled(
                    format!("{:<25} FAIL  {duration:>10}  {sha}", r.timestamp),
                    Style::default().fg(Color::Red),
                )),
                None => Line::from(format!("{:<25} ?     {duration:>10}  {sha}", r.timestamp)),
            }
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((app.scroll, 0)),
        area,
    );
}

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

fn render_footer(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let nav_hint = match app.active_tab {
        Tab::Overview | Tab::Flow => "tab/←→ switch tabs",
        _ => "tab/←→ switch tabs  •  j/k, PgUp/PgDn, g/G scroll",
    };
    let footer = Paragraph::new(format!(
        "q/Esc quit  •  a actions  •  {nav_hint}  •  refreshed {}",
        snapshot.refreshed_at
    ));
    frame.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{AgentTaskCounts, AgenticContext, ProjectInfo};

    fn empty_snapshot() -> Snapshot {
        Snapshot {
            refreshed_at: "12:34:56".to_string(),
            baseline: None,
            ci_run_count: 0,
            last_ci_passed: None,
            ci_duration_drift: None,
            ci_duration_history: Vec::new(),
            ci_passed_history: Vec::new(),
            records: Vec::new(),
            flow_status: None,
            flow_state: None,
            flow_conflict_resolver: ConflictResolverKind::default(),
            flow_auto_duration_history: Vec::new(),
            flow_auto_result_history: Vec::new(),
            flow_auto_conflicts_last: None,
            protocol_drift: None,
        }
    }

    fn test_app() -> App {
        App {
            active_tab: Tab::Overview,
            crate_names: vec!["taskit".to_string(), "taskit-tui".to_string()],
            project_info: ProjectInfo {
                name: "taskit".to_string(),
                root: "/workspace/taskit".to_string(),
                taskit_version: "0.8.0".to_string(),
                taskit_binary: Some("taskit".to_string()),
                git_sha: Some("0123456789abcdef".to_string()),
                rustc_version: Some("rustc 1.94.0".to_string()),
                cargo_version: Some("cargo 1.94.0".to_string()),
                os: "macos",
                arch: "aarch64",
                logical_cpus: 10,
                ci_step_names: vec!["fmt".to_string(), "clippy".to_string()],
                ci_gate_count: 1,
                protocol_surface_count: 8,
                agentic: AgenticContext {
                    instruction_files: vec!["AGENTS.md".to_string()],
                    config_files: vec![
                        "taskit.toml".to_string(),
                        ".ctx/godmode/tasks.yaml".to_string(),
                    ],
                    plan_count: 2,
                    tasks: Some(AgentTaskCounts {
                        pending: 3,
                        active: 1,
                        blocked: 0,
                        done: 81,
                    }),
                },
            },
            actions: ActionController::default(),
            scroll: 0,
        }
    }

    fn rendered_app(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let snapshot = empty_snapshot();

        terminal
            .draw(|frame| render(frame, app, &snapshot))
            .expect("render overview");

        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .fold(String::new(), |mut output, cell| {
                output.push_str(cell.symbol());
                output
            })
    }

    fn rendered_overview(width: u16, height: u16) -> String {
        rendered_app(&test_app(), width, height)
    }

    #[test]
    fn overview_renders_project_context_panels() {
        let output = rendered_overview(140, 42);

        for title in [
            "Project",
            "Workspace & Pipeline",
            "Agentic Context",
            "Health",
            "Activity",
        ] {
            assert!(output.contains(title), "missing overview panel: {title}");
        }
        for value in [
            "taskit",
            "macos/aarch64",
            "10 logical CPUs",
            "AGENTS.md",
            "3 pending",
            "2 steps, 1 gate",
            "a actions",
        ] {
            assert!(output.contains(value), "missing overview value: {value}");
        }
    }

    #[test]
    fn overview_renders_in_narrow_terminal() {
        let output = rendered_overview(64, 24);

        assert!(output.contains("Project"));
        assert!(output.contains("Health"));
    }

    #[test]
    fn actions_modal_lists_health_updates() {
        let mut app = test_app();
        app.actions.open();

        let output = rendered_app(&app, 120, 36);

        assert!(output.contains("Actions"));
        assert!(output.contains("Update health baseline"));
        assert!(output.contains("Update health + coverage"));
        assert!(output.contains("Enter run"));
    }
}
