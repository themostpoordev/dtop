use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::{
    cursor, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type DtopTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    pub terminal: DtopTerminal,
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        enable_raw_mode().context("enable terminal raw mode")?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, cursor::Hide) {
            let _ = disable_raw_mode();
            return Err(error).context("enter alternate screen");
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout)).context("create terminal")?;
        Ok(Self { terminal })
    }

    pub fn restore(&mut self) -> Result<()> {
        disable_raw_mode().context("disable terminal raw mode")?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)
            .context("restore terminal")?;
        self.terminal.show_cursor().context("show terminal cursor")
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
