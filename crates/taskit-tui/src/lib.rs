//! Terminal governance dashboard: workspace health, CI telemetry, drift
//! status, and a per-crate/history breakdown across a tabbed, scrollable
//! live-refreshing screen.
//!
//! All data is read from disk — the health baseline file and telemetry
//! NDJSON written by `taskit health` / `taskit ci` — so a refresh never
//! re-runs clippy, nextest, or cargo itself. The workspace crate list is
//! fetched once at startup (it shells out to `cargo metadata`), not on every
//! tick.

mod app;
mod snapshot;
mod ui;

pub use app::{App, Tab};
pub use snapshot::Snapshot;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use taskit_engine::ctx::Ctx;
use taskit_types::error::TaskitError;

const TICK: Duration = Duration::from_millis(500);

/// Install a panic hook that restores the terminal before the default hook
/// prints — the standard ratatui idiom, since a panic mid-draw would
/// otherwise strand the user in raw/alternate-screen mode with no visible
/// backtrace.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(panic_info);
    }));
}

/// Run the interactive dashboard until the user quits (`q`, `Esc`, `Ctrl-C`).
pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    install_panic_hook();
    enable_raw_mode().map_err(TaskitError::other)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(TaskitError::other)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(TaskitError::other)?;

    let result = event_loop(ctx, &mut terminal);

    // Best-effort teardown even if the loop errored, so a crash doesn't leave
    // the user's terminal in raw/alternate-screen mode.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

fn event_loop(
    ctx: &Ctx,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), TaskitError> {
    let mut app = App::new(ctx);

    loop {
        let snapshot = Snapshot::collect(ctx);
        app.clamp_scroll(&snapshot);
        terminal
            .draw(|frame| ui::render(frame, &app, &snapshot))
            .map_err(TaskitError::other)?;

        if event::poll(TICK).map_err(TaskitError::other)?
            && let Event::Key(key) = event::read().map_err(TaskitError::other)?
            && handle_key(&mut app, key.code, key.modifiers)
        {
            return Ok(());
        }
    }
}

/// Handle one key event, mutating `app`. Returns `true` if the app should
/// quit.
fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => return true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::PageDown => app.scroll_down_page(),
        KeyCode::PageUp => app.scroll_up_page(),
        KeyCode::Char('g') | KeyCode::Home => app.scroll_top(),
        KeyCode::Char('G') | KeyCode::End => app.scroll_bottom(),
        _ => {}
    }
    false
}
