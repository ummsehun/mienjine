use anyhow::Result;

use crate::{
    renderer::FrameBuffers,
    runtime::{
        app::set_runtime_panic_state,
        state::{cap_render_size, is_terminal_size_unstable},
    },
    scene::{
        estimate_cell_aspect_from_window, kitty_internal_resolution, KittyInternalResPreset,
        RenderConfig, RenderOutputMode,
    },
    terminal::TerminalSession,
};

pub(crate) fn set_runtime_panic_state_proxy(line: String) {
    set_runtime_panic_state(line);
}

pub(crate) fn is_retryable_io_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(|io_err| {
                matches!(
                    io_err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                )
            })
            .unwrap_or(false)
    })
}

pub(crate) fn detect_terminal_cell_aspect() -> Option<f32> {
    let ws = crossterm::terminal::window_size().ok()?;
    estimate_cell_aspect_from_window(ws.columns, ws.rows, ws.width, ws.height)
}

fn kitty_internal_cell_size(preset: KittyInternalResPreset) -> (u16, u16) {
    let (px_w, px_h) = kitty_internal_resolution(preset);
    let cols = ((u32::from(px_w)) / 2).max(1);
    let rows = ((u32::from(px_h)) / 4).max(1);
    let capped_cols = cols.min(u32::from(u16::MAX)) as u16;
    let capped_rows = rows.min(u32::from(u16::MAX)) as u16;
    (capped_cols.max(1), capped_rows.max(1))
}

fn kitty_internal_res_level(preset: KittyInternalResPreset) -> usize {
    match preset {
        KittyInternalResPreset::R640x360 => 0,
        KittyInternalResPreset::R854x480 => 1,
        KittyInternalResPreset::R1280x720 => 2,
    }
}

fn kitty_internal_res_from_level(level: usize) -> KittyInternalResPreset {
    match level {
        0 => KittyInternalResPreset::R640x360,
        1 => KittyInternalResPreset::R854x480,
        _ => KittyInternalResPreset::R1280x720,
    }
}

pub(crate) fn kitty_internal_res_for_lod(
    base: KittyInternalResPreset,
    lod_level: usize,
) -> KittyInternalResPreset {
    let base_level = kitty_internal_res_level(base);
    let target_level = base_level.saturating_sub(lod_level.min(2));
    kitty_internal_res_from_level(target_level)
}

fn desired_render_cells_for_mode(
    config: &RenderConfig,
    display_cells: (u16, u16),
    graphics_enabled: bool,
) -> (u16, u16) {
    if graphics_enabled && matches!(config.output_mode, RenderOutputMode::KittyHq) {
        let (target_w, target_h) = kitty_internal_cell_size(config.kitty_internal_res);
        let (target_w, target_h, _) = cap_render_size(target_w, target_h);
        (target_w.max(1), target_h.max(1))
    } else {
        display_cells
    }
}

pub(crate) fn resize_runtime_frame(
    terminal: &mut TerminalSession,
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    display_cells: (u16, u16),
    graphics_enabled: bool,
) -> (u16, u16) {
    let desired = desired_render_cells_for_mode(config, display_cells, graphics_enabled);
    if frame.width != desired.0 || frame.height != desired.1 {
        frame.resize(desired.0, desired.1);
        terminal.force_full_repaint();
    }
    desired
}

pub(crate) fn validated_terminal_size(terminal: &TerminalSession) -> Result<(u16, u16)> {
    let (w, h) = terminal.size()?;
    if !is_terminal_size_unstable(w, h) {
        return Ok((w, h));
    }
    let env_w = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0 && *v < u16::MAX);
    let env_h = std::env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0 && *v < u16::MAX);
    match (env_w, env_h) {
        (Some(width), Some(height)) if !is_terminal_size_unstable(width, height) => {
            Ok((width, height))
        }
        _ => anyhow::bail!(
            "terminal size unavailable (got {w}x{h}). set COLUMNS/LINES or use a real TTY terminal"
        ),
    }
}
