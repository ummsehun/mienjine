use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{
        BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen,
        LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    interfaces::cli::terminal_caps::ensure_tty,
    renderer::FrameBuffers,
    runtime::graphics_proto::{GraphicsPresentOptions, write_graphics_frame},
    runtime::terminal_diff::{build_diff_segments, quantize_rgb},
    scene::{
        AnsiQuantization, GraphicsProtocol, KittyCompression, KittyPipelineMode, KittyTransport,
        RecoverStrategy,
    },
};

pub use crate::interfaces::cli::terminal_caps::TerminalProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentMode {
    #[default]
    Diff,
    FullFallback,
}

pub struct TerminalSession {
    stdout: io::Stdout,
    alt_screen: bool,
    cursor_hidden: bool,
    raw_mode: bool,
    presenter: TerminalPresenter,
    sync_updates: bool,
}

impl TerminalSession {
    pub fn enter() -> Result<Self> {
        Self::enter_with_profile(TerminalProfile::detect())
    }

    pub fn enter_with_profile(profile: TerminalProfile) -> Result<Self> {
        ensure_tty()?;
        let alt_screen = profile.use_alt_screen;
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
            presenter: TerminalPresenter::default(),
            sync_updates: profile.use_sync_updates,
        })
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        Ok(size()?)
    }

    pub fn set_present_mode(&mut self, mode: PresentMode) {
        self.presenter.mode = mode;
        self.presenter.force_full_repaint = true;
    }

    pub fn present_mode(&self) -> PresentMode {
        self.presenter.mode
    }

    pub fn force_full_repaint(&mut self) {
        self.presenter.force_full_repaint = true;
    }

    pub fn draw_frame(&mut self, text: &str) -> Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, 0),
            Clear(ClearType::All),
            Print("\x1b[0m"),
            Print(text)
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn draw_frame_ansi(&mut self, text: &str) -> Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, 0),
            Clear(ClearType::All),
            Print(text),
            Print("\x1b[0m")
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn present(
        &mut self,
        frame: &FrameBuffers,
        use_ansi: bool,
        quantization: AnsiQuantization,
    ) -> Result<()> {
        if self.presenter.width != frame.width || self.presenter.height != frame.height {
            self.presenter.resize(frame.width, frame.height);
            self.presenter.force_full_repaint = true;
        }

        if self.presenter.last_has_color != use_ansi {
            self.presenter.force_full_repaint = true;
        }
        if self.presenter.last_quantization != quantization {
            self.presenter.force_full_repaint = true;
        }

        if matches!(self.presenter.mode, PresentMode::FullFallback)
            || self.presenter.force_full_repaint
        {
            self.draw_full_frame(frame, use_ansi, quantization)?;
            self.presenter
                .capture_snapshot(frame, use_ansi, quantization);
            self.presenter.force_full_repaint = false;
            return Ok(());
        }

        let segments = build_diff_segments(
            frame,
            &self.presenter.last_glyphs,
            &self.presenter.last_rgb,
            use_ansi,
            quantization,
        );

        if segments.is_empty() {
            return Ok(());
        }

        if self.sync_updates {
            queue!(self.stdout, BeginSynchronizedUpdate)?;
        }
        queue!(self.stdout, Print("\x1b[0m"))?;

        for segment in &segments {
            queue!(
                self.stdout,
                MoveTo(segment.x, segment.y),
                Print(&segment.payload)
            )?;
            for idx in segment.start_idx..segment.end_idx_exclusive {
                self.presenter.last_glyphs[idx] = frame.glyphs[idx];
                self.presenter.last_rgb[idx] = if use_ansi {
                    quantize_rgb(frame.fg_rgb[idx], quantization)
                } else {
                    [255, 255, 255]
                };
            }
        }

        if self.sync_updates {
            queue!(self.stdout, EndSynchronizedUpdate)?;
        }
        self.stdout.flush()?;
        self.presenter.last_has_color = use_ansi;
        self.presenter.last_quantization = quantization;
        Ok(())
    }

    pub fn present_graphics(
        &mut self,
        frame: &FrameBuffers,
        protocol: GraphicsProtocol,
        transport: KittyTransport,
        compression: KittyCompression,
        pipeline_mode: KittyPipelineMode,
        recover_strategy: RecoverStrategy,
        scale: f32,
        display_cells: (u16, u16),
        force_reupload: bool,
    ) -> Result<()> {
        write_graphics_frame(
            &mut self.stdout,
            frame,
            protocol,
            GraphicsPresentOptions {
                transport,
                compression,
                pipeline_mode,
                recover_strategy,
                scale,
                display_cells: Some(display_cells),
                force_reupload,
            },
        )?;
        // Force a full repaint if/when we fall back to text mode.
        self.presenter.force_full_repaint = true;
        Ok(())
    }

    fn draw_full_frame(
        &mut self,
        frame: &FrameBuffers,
        use_ansi: bool,
        quantization: AnsiQuantization,
    ) -> Result<()> {
        let mut text = String::new();
        if use_ansi {
            frame.write_ansi_text(&mut text, quantization);
            queue!(
                self.stdout,
                MoveTo(0, 0),
                Clear(ClearType::All),
                Print(text),
                Print("\x1b[0m")
            )?;
        } else {
            frame.write_text(&mut text);
            queue!(
                self.stdout,
                MoveTo(0, 0),
                Clear(ClearType::All),
                Print("\x1b[0m"),
                Print(text)
            )?;
        }
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
        let _ = queue!(self.stdout, Print("\x1b[0m"));
        let _ = self.stdout.flush();
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

#[derive(Debug)]
struct TerminalPresenter {
    mode: PresentMode,
    width: u16,
    height: u16,
    last_glyphs: Vec<char>,
    last_rgb: Vec<[u8; 3]>,
    last_has_color: bool,
    last_quantization: AnsiQuantization,
    force_full_repaint: bool,
}

impl Default for TerminalPresenter {
    fn default() -> Self {
        Self {
            mode: PresentMode::Diff,
            width: 0,
            height: 0,
            last_glyphs: Vec::new(),
            last_rgb: Vec::new(),
            last_has_color: false,
            last_quantization: AnsiQuantization::Q216,
            force_full_repaint: false,
        }
    }
}

impl TerminalPresenter {
    fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width).saturating_mul(usize::from(height));
        self.last_glyphs.resize(size, ' ');
        self.last_glyphs.fill(' ');
        self.last_rgb.resize(size, [255, 255, 255]);
        self.last_rgb.fill([255, 255, 255]);
        self.last_has_color = false;
    }

    fn capture_snapshot(
        &mut self,
        frame: &FrameBuffers,
        use_ansi: bool,
        quantization: AnsiQuantization,
    ) {
        self.width = frame.width;
        self.height = frame.height;
        self.last_glyphs.clone_from(&frame.glyphs);
        self.last_rgb.resize(frame.fg_rgb.len(), [255, 255, 255]);
        if use_ansi {
            for (dst, src) in self.last_rgb.iter_mut().zip(frame.fg_rgb.iter().copied()) {
                *dst = quantize_rgb(src, quantization);
            }
        } else {
            self.last_rgb.fill([255, 255, 255]);
        }
        self.last_has_color = use_ansi;
        self.last_quantization = quantization;
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
        Self::enter_with_profile(TerminalProfile::detect())
    }

    pub fn enter_with_profile(profile: TerminalProfile) -> Result<Self> {
        ensure_tty()?;
        let alt_screen = profile.use_alt_screen;
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
        let _ = execute!(backend, Print("\x1b[0m"));
        let _ = backend.flush();
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presenter_resize_invalidates_previous_snapshot() {
        let mut presenter = TerminalPresenter::default();
        presenter.capture_snapshot(&FrameBuffers::new(2, 1), false, AnsiQuantization::Q216);
        presenter.resize(4, 2);
        assert!(presenter.force_full_repaint || presenter.last_glyphs.len() == 8);
        assert_eq!(presenter.last_glyphs.len(), 8);
        assert_eq!(presenter.last_rgb.len(), 8);
    }
}
