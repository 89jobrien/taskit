//! Widget layout for the dashboard frame: a tab bar plus a per-tab body.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Tabs};

use crate::app::{App, Tab};
use crate::snapshot::Snapshot;

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
        Tab::Overview => render_overview(frame, chunks[1], snapshot),
        Tab::Crates => render_crates(frame, chunks[1], app),
        Tab::History => render_history(frame, chunks[1], app, snapshot),
    }

    render_footer(frame, chunks[2], app, snapshot);
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

fn render_overview(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_health(frame, columns[0], snapshot);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(columns[1]);
    render_telemetry(frame, right[0], snapshot);
    render_duration_sparkline(frame, right[1], snapshot);
    render_pass_sparkline(frame, right[2], snapshot);
}

fn render_health(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let Some(b) = &snapshot.baseline else {
        let block = Block::default()
            .title("Workspace Health")
            .borders(Borders::ALL);
        let lines = vec![
            Line::from("No health baseline found."),
            Line::from("Run `taskit health --update` to create one."),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
        return;
    };

    let outer = Block::default()
        .title("Workspace Health")
        .borders(Borders::ALL);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let pass_ratio = if b.tests.total == 0 {
        0.0
    } else {
        b.tests.passed as f64 / b.tests.total as f64
    };
    let gauge_color = if b.tests.failed > 0 {
        Color::Red
    } else {
        Color::Green
    };
    frame.render_widget(
        Gauge::default()
            .block(Block::default().title("Tests passing"))
            .gauge_style(Style::default().fg(gauge_color))
            .ratio(pass_ratio)
            .label(format!(
                "{}/{} passed, {} failed",
                b.tests.passed, b.tests.total, b.tests.failed
            )),
        rows[0],
    );

    let clippy_clean = b.clippy.warnings == 0 && b.clippy.errors == 0;
    frame.render_widget(
        Gauge::default()
            .block(Block::default().title("Clippy"))
            .gauge_style(Style::default().fg(if clippy_clean {
                Color::Green
            } else {
                Color::Yellow
            }))
            .ratio(if clippy_clean { 1.0 } else { 0.0 })
            .label(format!(
                "{} warnings, {} errors",
                b.clippy.warnings, b.clippy.errors
            )),
        rows[1],
    );

    let lines = vec![
        Line::from(format!("TODO/FIXME:  {}", b.todo_fixme)),
        Line::from(format!("Crates:      {}", b.crates)),
        Line::from(format!(
            "Version:     {} (consistent: {})",
            b.version, b.versions_consistent
        )),
        Line::from(format!("Baseline:    {}", b.date)),
    ];
    frame.render_widget(Paragraph::new(lines), rows[2]);
}

fn render_telemetry(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title("CI Telemetry & Drift (7d)")
        .borders(Borders::ALL);

    let mut lines = vec![Line::from(format!(
        "CI runs recorded: {}",
        snapshot.ci_run_count
    ))];

    lines.push(match snapshot.last_ci_passed {
        Some(true) => Line::from(Span::styled(
            "Last CI run:  PASS",
            Style::default().fg(Color::Green),
        )),
        Some(false) => Line::from(Span::styled(
            "Last CI run:  FAIL",
            Style::default().fg(Color::Red),
        )),
        None => Line::from("Last CI run:  (no data)"),
    });

    lines.push(match &snapshot.ci_duration_drift {
        Some(d) if d.regressed => Line::from(Span::styled(
            format!(
                "ci_duration_ms: DRIFT ({:.0}ms, {:+.1}% vs mean)",
                d.current, d.delta_pct
            ),
            Style::default().fg(Color::Red),
        )),
        Some(d) => Line::from(Span::styled(
            format!(
                "ci_duration_ms: stable ({:.0}ms, {:+.1}% vs mean)",
                d.current, d.delta_pct
            ),
            Style::default().fg(Color::Green),
        )),
        None => Line::from("ci_duration_ms: not enough history yet"),
    });

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_duration_sparkline(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title("ci_duration_ms trend")
        .borders(Borders::ALL);
    if snapshot.ci_duration_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No CI runs recorded yet — run `taskit ci`.").block(block),
            area,
        );
        return;
    }
    let color = match &snapshot.ci_duration_drift {
        Some(d) if d.regressed => Color::Red,
        _ => Color::Cyan,
    };
    frame.render_widget(
        Sparkline::default()
            .block(block)
            .data(&snapshot.ci_duration_history)
            .style(Style::default().fg(color)),
        area,
    );
}

fn render_pass_sparkline(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title("ci_passed trend (1=pass, 0=fail)")
        .borders(Borders::ALL);
    if snapshot.ci_passed_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No CI runs recorded yet — run `taskit ci`.").block(block),
            area,
        );
        return;
    }
    frame.render_widget(
        Sparkline::default()
            .block(block)
            .data(&snapshot.ci_passed_history)
            .style(Style::default().fg(Color::Green)),
        area,
    );
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

fn render_footer(frame: &mut Frame, area: Rect, app: &App, snapshot: &Snapshot) {
    let nav_hint = match app.active_tab {
        Tab::Overview => "tab/←→ switch tabs",
        _ => "tab/←→ switch tabs  •  j/k, PgUp/PgDn, g/G scroll",
    };
    let footer = Paragraph::new(format!(
        "q/Esc quit  •  {nav_hint}  •  refreshed {}",
        snapshot.refreshed_at
    ));
    frame.render_widget(footer, area);
}
