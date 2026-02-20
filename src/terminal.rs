use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{
        BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode, size,
    },
};

pub struct TerminalSession {
    stdout: io::Stdout,
}

impl TerminalSession {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        Ok(size()?)
    }

    pub fn draw_frame(&mut self, text: &str) -> Result<()> {
        queue!(
            self.stdout,
            BeginSynchronizedUpdate,
            MoveTo(0, 0),
            Print(text),
            EndSynchronizedUpdate
        )?;
        self.stdout.flush()?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = queue!(self.stdout, Show, LeaveAlternateScreen);
        let _ = self.stdout.flush();
        let _ = disable_raw_mode();
    }
}
