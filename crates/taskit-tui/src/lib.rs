//! Terminal governance dashboard: workspace health, CI telemetry, and drift
//! status on a single live-refreshing screen.
//!
//! All data is read from disk — the health baseline file and telemetry
//! NDJSON written by `taskit health` / `taskit ci` — so a refresh never
//! re-runs clippy, nextest, or cargo itself.

mod snapshot;
mod ui;

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
    loop {
        let snapshot = Snapshot::collect(ctx);
        terminal
            .draw(|frame| ui::render(frame, &snapshot))
            .map_err(TaskitError::other)?;

        if event::poll(TICK).map_err(TaskitError::other)?
            && let Event::Key(key) = event::read().map_err(TaskitError::other)?
        {
            let quit = matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                || (key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL));
            if quit {
                return Ok(());
            }
        }
    }
}
