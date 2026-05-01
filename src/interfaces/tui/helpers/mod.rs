use std::{fs::File, io::BufReader, path::Path};

use crossterm::terminal::window_size;
use ratatui::prelude::*;
use rodio::{Decoder, Source};

use crate::{
    loader,
    runtime::config::UiLanguage,
    scene::{SyncSpeedMode, estimate_cell_aspect_from_window},
};

pub(crate) use crate::shared::constants::{SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_STEP_MS};

use super::start_ui::UiBreakpoint;

pub(crate) const MIN_WIDTH: u16 = 60;
pub(crate) const MIN_HEIGHT: u16 = 18;
pub(crate) const START_FPS_OPTIONS: [u32; 9] = [0, 15, 20, 24, 30, 40, 60, 90, 120];
pub(crate) const RENDER_FIELD_COUNT: usize = 33;
pub(crate) const QUICK_RENDER_FIELD_COUNT: usize = 12;
pub(crate) const RATATUI_SAFE_MAX_CELLS: u32 = (u16::MAX as u32) - 1;

pub(crate) fn clamp_ratatui_area(area: Rect) -> Rect {
    let cells = (area.width as u32).saturating_mul(area.height as u32);
    if cells <= RATATUI_SAFE_MAX_CELLS {
        return area;
    }
    let aspect = if area.height == 0 {
        1.0
    } else {
        (area.width as f32 / area.height as f32).max(0.1)
    };
    let h = ((RATATUI_SAFE_MAX_CELLS as f32 / aspect).sqrt().floor() as u16).max(1);
    let w = ((h as f32 * aspect).floor() as u16).max(1);
    Rect {
        x: area.x,
        y: area.y,
        width: w,
        height: h,
    }
}

pub(crate) fn tr<'a>(lang: UiLanguage, ko: &'a str, en: &'a str) -> &'a str {
    match lang {
        UiLanguage::Ko => ko,
        UiLanguage::En => en,
    }
}

pub(crate) fn cycle_index(index: &mut usize, len: usize, delta: i32) {
    if len == 0 {
        *index = 0;
        return;
    }
    if delta > 0 {
        *index = (*index + 1) % len;
    } else if delta < 0 {
        *index = if *index == 0 { len - 1 } else { *index - 1 };
    }
}

pub(crate) fn closest_u32_index(value: u32, options: &[u32]) -> usize {
    options
        .iter()
        .enumerate()
        .min_by_key(|(_, option)| option.abs_diff(value))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub(crate) fn breakpoint_for(width: u16, height: u16) -> UiBreakpoint {
    if width >= 140 && height >= 40 {
        UiBreakpoint::Wide
    } else if width >= 100 && height >= 28 {
        UiBreakpoint::Normal
    } else {
        UiBreakpoint::Compact
    }
}

pub(crate) fn format_mib(bytes: u64) -> String {
    let mib = (bytes as f64) / (1024.0 * 1024.0);
    format!("{mib:.1} MiB")
}

pub(crate) fn duration_label(seconds: Option<f32>) -> String {
    seconds
        .map(|v| format!("{v:.3}s"))
        .unwrap_or_else(|| "n/a".to_owned())
}

pub(crate) fn fps_label(fps: u32, lang: UiLanguage) -> String {
    if fps == 0 {
        tr(lang, "무제한", "Unlimited").to_owned()
    } else {
        fps.to_string()
    }
}

pub(crate) fn detect_terminal_cell_aspect() -> Option<f32> {
    let ws = window_size().ok()?;
    estimate_cell_aspect_from_window(ws.columns, ws.rows, ws.width, ws.height)
}

pub(crate) fn inspect_clip_duration(path: &Path, anim_selector: Option<&str>) -> Option<f32> {
    let scene = loader::load_gltf(path).ok()?;
    if scene.animations.is_empty() {
        return None;
    }
    if let Some(selector) = anim_selector {
        let index = scene.animation_index_by_selector(Some(selector))?;
        return scene.animations.get(index).map(|clip| clip.duration);
    }
    scene.animations.first().map(|clip| clip.duration)
}

pub(crate) fn inspect_motion_duration(path: &Path) -> Option<f32> {
    crate::assets::vmd_motion::parse_vmd_motion(path)
        .ok()
        .map(|motion| motion.duration_secs())
}

pub(crate) fn inspect_audio_duration(path: &Path) -> Option<f32> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    decoder.total_duration().map(|d| d.as_secs_f32())
}

pub(crate) fn compute_duration_fit_factor(
    clip_duration_secs: Option<f32>,
    audio_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> f32 {
    if !matches!(mode, SyncSpeedMode::AutoDurationFit) {
        return 1.0;
    }
    let Some(clip) = clip_duration_secs else {
        return 1.0;
    };
    let Some(audio) = audio_duration_secs else {
        return 1.0;
    };
    if clip <= f32::EPSILON || audio <= f32::EPSILON {
        return 1.0;
    }
    let factor = clip / audio;
    if (0.85..=1.15).contains(&factor) {
        factor
    } else {
        1.0
    }
}

pub(crate) fn aspect_preview_ascii(width: u16, height: u16, aspect: f32) -> String {
    let w = width.max(12) as usize;
    let h = height.max(6) as usize;
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    let radius = (w.min(h) as f32) * 0.35;
    let mut out = String::with_capacity(w.saturating_mul(h + 1));

    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / radius;
            let dy = (y as f32 - cy) / radius;
            let d = ((dx * aspect).powi(2) + dy.powi(2)).sqrt();
            let ch = if (d - 1.0).abs() < 0.08 {
                '@'
            } else if d < 1.0 {
                '.'
            } else {
                ' '
            };
            out.push(ch);
        }
        if y + 1 < h {
            out.push('\n');
        }
    }
    out
}
