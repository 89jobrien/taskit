//! Widget layout for the dashboard frame.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

use crate::snapshot::Snapshot;

pub fn render(frame: &mut Frame, snapshot: &Snapshot) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_title(frame, chunks[0]);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    render_health(frame, columns[0], snapshot);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(columns[1]);
    render_telemetry(frame, right[0], snapshot);
    render_sparkline(frame, right[1], snapshot);

    render_footer(frame, chunks[2], snapshot);
}

fn render_title(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new("taskit — real-time governance dashboard")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, area);
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

fn render_sparkline(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
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

fn render_footer(frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let footer = Paragraph::new(format!(
        "q/Esc to quit  •  refreshed {}",
        snapshot.refreshed_at
    ));
    frame.render_widget(footer, area);
}
