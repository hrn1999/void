use crate::app::state::AppState;
use crate::config::Config;
use anyhow::Context;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

pub mod theme;
pub mod widgets;

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    terminal: TuiTerminal,
}

impl TerminalGuard {
    pub fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("enable raw mode")?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("enter alt screen + mouse capture")?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).context("create terminal")?;

        Ok(Self { terminal })
    }

    pub fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort cleanup; don't panic in Drop.
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

pub fn draw(terminal: &mut TuiTerminal, cfg: &Config, state: &mut AppState) -> anyhow::Result<()> {
    // Clear expired toasts
    if let Some(toast) = &state.toast
        && toast.is_expired() {
            state.toast = None;
        }

    terminal
        .draw(|f| {
            widgets::root::render(f, cfg, state);
        })
        .context("terminal draw")?;
    Ok(())
}

