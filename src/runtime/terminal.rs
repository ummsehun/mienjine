use std::io::{self, IsTerminal, Write};

use anyhow::{Result, bail};
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

use crate::renderer::FrameBuffers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentMode {
    Diff,
    FullFallback,
}

impl Default for PresentMode {
    fn default() -> Self {
        Self::Diff
    }
}

#[derive(Debug)]
struct DiffSegment {
    x: u16,
    y: u16,
    start_idx: usize,
    end_idx_exclusive: usize,
    payload: String,
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
            presenter: TerminalPresenter::default(),
            sync_updates: should_use_sync_updates(),
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

    pub fn present(&mut self, frame: &FrameBuffers, use_ansi: bool) -> Result<()> {
        if self.presenter.width != frame.width || self.presenter.height != frame.height {
            self.presenter.resize(frame.width, frame.height);
            self.presenter.force_full_repaint = true;
        }

        if self.presenter.last_has_color != use_ansi {
            self.presenter.force_full_repaint = true;
        }

        if matches!(self.presenter.mode, PresentMode::FullFallback)
            || self.presenter.force_full_repaint
        {
            self.draw_full_frame(frame, use_ansi)?;
            self.presenter.capture_snapshot(frame, use_ansi);
            self.presenter.force_full_repaint = false;
            return Ok(());
        }

        let segments = build_diff_segments(
            frame,
            &self.presenter.last_glyphs,
            &self.presenter.last_rgb,
            use_ansi,
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
                    quantize_rgb16(frame.fg_rgb[idx])
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
        Ok(())
    }

    fn draw_full_frame(&mut self, frame: &FrameBuffers, use_ansi: bool) -> Result<()> {
        let mut text = String::new();
        if use_ansi {
            frame.write_ansi_text(&mut text);
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

#[derive(Debug, Default)]
struct TerminalPresenter {
    mode: PresentMode,
    width: u16,
    height: u16,
    last_glyphs: Vec<char>,
    last_rgb: Vec<[u8; 3]>,
    last_has_color: bool,
    force_full_repaint: bool,
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

    fn capture_snapshot(&mut self, frame: &FrameBuffers, use_ansi: bool) {
        self.width = frame.width;
        self.height = frame.height;
        self.last_glyphs.clone_from(&frame.glyphs);
        self.last_rgb.resize(frame.fg_rgb.len(), [255, 255, 255]);
        if use_ansi {
            for (dst, src) in self.last_rgb.iter_mut().zip(frame.fg_rgb.iter().copied()) {
                *dst = quantize_rgb16(src);
            }
        } else {
            self.last_rgb.fill([255, 255, 255]);
        }
        self.last_has_color = use_ansi;
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
        let _ = execute!(backend, Print("\x1b[0m"));
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

fn should_use_sync_updates() -> bool {
    let disable = std::env::var("GASCII_NO_SYNC_UPDATES")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    !disable
}

fn build_diff_segments(
    frame: &FrameBuffers,
    previous_glyphs: &[char],
    previous_rgb: &[[u8; 3]],
    use_ansi: bool,
) -> Vec<DiffSegment> {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    for y in 0..height {
        let row_start = y * width;
        let row_end = row_start + width;
        let mut x = 0usize;
        while row_start + x < row_end {
            let idx = row_start + x;
            if !cell_changed(idx, frame, previous_glyphs, previous_rgb, use_ansi) {
                x += 1;
                continue;
            }

            let run_start_x = x;
            let run_start_idx = idx;
            let mut payload = String::new();
            let mut current_rgb: Option<[u8; 3]> = None;
            while row_start + x < row_end {
                let ridx = row_start + x;
                if !cell_changed(ridx, frame, previous_glyphs, previous_rgb, use_ansi) {
                    break;
                }
                if use_ansi {
                    let rgb = quantize_rgb16(frame.fg_rgb[ridx]);
                    if current_rgb != Some(rgb) {
                        push_fg_ansi(&mut payload, rgb);
                        current_rgb = Some(rgb);
                    }
                }
                payload.push(frame.glyphs[ridx]);
                x += 1;
            }
            if use_ansi {
                payload.push_str("\x1b[0m");
            }
            out.push(DiffSegment {
                x: run_start_x as u16,
                y: y as u16,
                start_idx: run_start_idx,
                end_idx_exclusive: row_start + x,
                payload,
            });
        }
    }
    out
}

fn cell_changed(
    idx: usize,
    frame: &FrameBuffers,
    previous_glyphs: &[char],
    previous_rgb: &[[u8; 3]],
    use_ansi: bool,
) -> bool {
    if frame.glyphs.get(idx).copied().unwrap_or(' ')
        != previous_glyphs.get(idx).copied().unwrap_or(' ')
    {
        return true;
    }
    if use_ansi {
        let curr = quantize_rgb16(frame.fg_rgb.get(idx).copied().unwrap_or([255, 255, 255]));
        let prev = previous_rgb.get(idx).copied().unwrap_or([255, 255, 255]);
        return curr != prev;
    }
    false
}

fn push_fg_ansi(out: &mut String, rgb: [u8; 3]) {
    use std::fmt::Write as _;
    let _ = write!(out, "\x1b[38;2;{};{};{}m", rgb[0], rgb[1], rgb[2]);
}

fn quantize_rgb16(rgb: [u8; 3]) -> [u8; 3] {
    const PALETTE_16: [[u8; 3]; 16] = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [192, 192, 192],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];

    let mut best = PALETTE_16[0];
    let mut best_dist = u32::MAX;
    for candidate in PALETTE_16 {
        let dr = rgb[0] as i32 - candidate[0] as i32;
        let dg = rgb[1] as i32 - candidate[1] as i32;
        let db = rgb[2] as i32 - candidate[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = candidate;
        }
    }
    best
}

pub fn supports_truecolor() -> bool {
    let color_term = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if color_term.contains("truecolor") || color_term.contains("24bit") {
        return true;
    }
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    term.contains("direct")
        || term.contains("kitty")
        || term.contains("wezterm")
        || term.contains("ghostty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_segments_include_only_changed_runs() {
        let mut frame = FrameBuffers::new(5, 1);
        frame.glyphs.clone_from_slice(&['a', 'b', 'c', 'd', 'e']);
        frame.fg_rgb.fill([255, 255, 255]);
        let prev_glyphs = vec!['a', 'x', 'c', 'd', 'e'];
        let prev_rgb = vec![[255, 255, 255]; 5];

        let segments = build_diff_segments(&frame, &prev_glyphs, &prev_rgb, false);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].x, 1);
        assert_eq!(segments[0].payload, "b");
    }

    #[test]
    fn ansi_diff_quantizes_colors_before_compare() {
        let mut frame = FrameBuffers::new(2, 1);
        frame.glyphs.clone_from_slice(&['@', '#']);
        frame.fg_rgb[0] = [250, 10, 10];
        frame.fg_rgb[1] = [240, 15, 20];

        let prev_glyphs = vec!['@', '#'];
        let prev_rgb = vec![quantize_rgb16([255, 0, 0]), quantize_rgb16([255, 0, 0])];

        let segments = build_diff_segments(&frame, &prev_glyphs, &prev_rgb, true);
        assert!(segments.is_empty());
    }
}
