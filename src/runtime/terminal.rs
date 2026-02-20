use std::io::{self, IsTerminal, Write};

use anyhow::{Result, bail};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub struct TerminalSession {
    stdout: io::Stdout,
    alt_screen: bool,
    cursor_hidden: bool,
    raw_mode: bool,
}

impl TerminalSession {
    pub fn enter() -> Result<Self> {
        ensure_tty()?;
        let alt_screen = should_use_alt_screen();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if alt_screen {
            execute!(stdout, EnterAlternateScreen, Hide)?;
        } else {
            execute!(stdout, Hide, Clear(ClearType::All), MoveTo(0, 0))?;
        }
        Ok(Self {
            stdout,
            alt_screen,
            cursor_hidden: true,
            raw_mode: true,
        })
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        Ok(size()?)
    }

    pub fn draw_frame(&mut self, text: &str) -> Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, 0),
            Clear(ClearType::All),
            Print(text)
        )?;
        self.stdout.flush()?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.cursor_hidden {
            let _ = queue!(self.stdout, Show);
        }
        if self.alt_screen {
            let _ = queue!(self.stdout, LeaveAlternateScreen);
        }
        let _ = self.stdout.flush();
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

pub struct RatatuiSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    alt_screen: bool,
    cursor_hidden: bool,
    raw_mode: bool,
}

impl RatatuiSession {
    pub fn enter() -> Result<Self> {
        ensure_tty()?;
        let alt_screen = should_use_alt_screen();
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        if alt_screen {
            execute!(stdout, EnterAlternateScreen, Hide)?;
        } else {
            execute!(stdout, Hide, Clear(ClearType::All), MoveTo(0, 0))?;
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            alt_screen,
            cursor_hidden: true,
            raw_mode: true,
        })
    }

    pub fn draw<F>(&mut self, draw_fn: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal.draw(draw_fn)?;
        Ok(())
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        let area = self.terminal.size()?;
        Ok((area.width, area.height))
    }
}

impl Drop for RatatuiSession {
    fn drop(&mut self) {
        let backend = self.terminal.backend_mut();
        if self.cursor_hidden {
            let _ = execute!(backend, Show);
        }
        if self.alt_screen {
            let _ = execute!(backend, LeaveAlternateScreen);
        }
        let _ = backend.flush();
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

fn ensure_tty() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("interactive TUI requires a real terminal (TTY). run directly in Ghostty/Terminal.");
    }
    Ok(())
}

fn should_use_alt_screen() -> bool {
    let force_no_alt = std::env::var("GASCII_NO_ALT_SCREEN")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let enable_alt = std::env::var("GASCII_ALT_SCREEN")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    enable_alt && !force_no_alt
}
