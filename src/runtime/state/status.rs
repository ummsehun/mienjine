use crate::{
    renderer::FrameBuffers,
    scene::{BrailleProfile, CinematicCameraMode, ColorMode},
};

use super::RuntimeContrastPreset;

pub(crate) fn format_runtime_status(
    sync_offset_ms: i32,
    sync_speed: f32,
    effective_aspect: f32,
    contrast: RuntimeContrastPreset,
    braille_profile: BrailleProfile,
    color_mode: ColorMode,
    cinematic_mode: CinematicCameraMode,
    reactive_gain: f32,
    exposure_bias: f32,
    stage_level: u8,
    center_lock: bool,
    lod_level: usize,
    target_ms: f32,
    frame_ema_ms: f32,
    sync_profile_hit: Option<bool>,
    sync_profile_dirty: bool,
    drift_ema: f32,
    hard_snap_count: u32,
    notice: Option<&str>,
) -> String {
    let profile_label = match sync_profile_hit {
        Some(true) => "hit",
        Some(false) => "miss",
        None => "off",
    };
    let core = format!(
        "offset={sync_offset_ms}ms  speed={sync_speed:.4}x  aspect={effective_aspect:.3}  contrast={}  braille={:?}  color={:?}  camera={:?}  gain={reactive_gain:.2}  exp={exposure_bias:+.2}  stage={}  center={}  lod={}  target={target_ms:.1}ms  ema={frame_ema_ms:.1}ms  profile={}{}  drift={drift_ema:.4}  snaps={hard_snap_count}",
        contrast.label(),
        braille_profile,
        color_mode,
        cinematic_mode,
        stage_level,
        if center_lock { "on" } else { "off" },
        lod_level,
        profile_label,
        if sync_profile_dirty { "*" } else { "" },
    );
    if let Some(extra) = notice {
        format!("{core}  note={extra}")
    } else {
        core
    }
}

pub(crate) fn overlay_osd(frame: &mut FrameBuffers, text: &str) {
    if frame.width == 0 || frame.height == 0 {
        return;
    }
    let width = usize::from(frame.width);
    let y = usize::from(frame.height.saturating_sub(1));
    let row_start = y * width;
    let row_end = row_start + width;
    for glyph in &mut frame.glyphs[row_start..row_end] {
        *glyph = ' ';
    }
    for color in &mut frame.fg_rgb[row_start..row_end] {
        *color = [235, 235, 235];
    }
    for (i, ch) in text.chars().take(width).enumerate() {
        frame.glyphs[row_start + i] = ch;
    }
}
