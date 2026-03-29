//! Braille rendering helpers including thresholds, composition, and safe visibility state.

use crate::scene::{BrailleProfile, ClarityProfile, ColorMode, RenderConfig};

use super::{FrameBuffers, RenderScratch, ThemePalette};

#[derive(Debug, Clone, Copy)]
pub struct BrailleThresholds {
    pub on_threshold: f32,
    pub min_visible: f32,
    pub floor: f32,
    pub gamma: f32,
}

pub fn braille_thresholds(
    profile: BrailleProfile,
    clarity: ClarityProfile,
    safe_boost: bool,
) -> BrailleThresholds {
    let clarity_delta = match clarity {
        ClarityProfile::Balanced => 0.0,
        ClarityProfile::Sharp => -0.01,
        ClarityProfile::Extreme => -0.02,
    };
    match profile {
        BrailleProfile::Safe => {
            let mut value = BrailleThresholds {
                on_threshold: (0.10_f32 + clarity_delta).clamp(0.04, 0.20),
                min_visible: 0.06,
                floor: 0.14,
                gamma: 0.82,
            };
            if safe_boost {
                value.on_threshold = (value.on_threshold - 0.02).clamp(0.04, 0.20);
                value.min_visible = (value.min_visible - 0.015).clamp(0.02, 0.20);
                value.floor = (value.floor + 0.03).clamp(0.04, 0.38);
            }
            value
        }
        BrailleProfile::Normal => BrailleThresholds {
            on_threshold: (0.13_f32 + clarity_delta).clamp(0.05, 0.24),
            min_visible: 0.09,
            floor: 0.10,
            gamma: 0.90,
        },
        BrailleProfile::Dense => BrailleThresholds {
            on_threshold: (0.16_f32 + clarity_delta).clamp(0.06, 0.26),
            min_visible: 0.12,
            floor: 0.07,
            gamma: 0.98,
        },
    }
}

pub fn compose_braille_cells(
    frame: &mut FrameBuffers,
    subpixels: &super::BrailleSubpixelBuffers,
    config: &RenderConfig,
    palette: ThemePalette,
    threshold: BrailleThresholds,
) {
    use crate::render::renderer_color::model_color_for_intensity;

    if frame.width == 0 || frame.height == 0 {
        return;
    }
    const MAP: [(u16, u16, u8); 8] = [
        (0, 0, 0x01),
        (0, 1, 0x02),
        (0, 2, 0x04),
        (1, 0, 0x08),
        (1, 1, 0x10),
        (1, 2, 0x20),
        (0, 3, 0x40),
        (1, 3, 0x80),
    ];
    let fw = usize::from(frame.width);
    let sw = usize::from(subpixels.width);
    for y in 0..usize::from(frame.height) {
        for x in 0..usize::from(frame.width) {
            let mut mask = 0_u8;
            let mut max_intensity = 0.0_f32;
            let mut best_bit = 0_u8;
            let mut best_depth = f32::INFINITY;
            let mut best_color = palette.highlight;
            for (ox, oy, bit) in MAP {
                let sx = x * 2 + usize::from(ox);
                let sy = y * 4 + usize::from(oy);
                if sx >= sw || sy >= usize::from(subpixels.height) {
                    continue;
                }
                let sidx = sy * sw + sx;
                let intensity = subpixels.intensity[sidx];
                if intensity >= threshold.on_threshold {
                    mask |= bit;
                }
                if intensity > max_intensity {
                    max_intensity = intensity;
                    best_bit = bit;
                    best_depth = subpixels.depth[sidx];
                    best_color = subpixels.color_rgb[sidx];
                }
            }
            if mask == 0 && max_intensity >= threshold.min_visible {
                mask = best_bit;
            }
            if matches!(config.braille_profile, BrailleProfile::Safe)
                && mask != 0
                && mask.count_ones() <= 1
                && max_intensity >= threshold.min_visible * 1.25
            {
                mask |= safe_neighbor_bit(best_bit);
            }
            if mask == 0 {
                continue;
            }
            let idx = y * fw + x;
            frame.glyphs[idx] = char::from_u32(0x2800 + mask as u32).unwrap_or(' ');
            frame.depth[idx] = best_depth;
            if matches!(config.color_mode, ColorMode::Ansi) {
                frame.fg_rgb[idx] = if best_color == [0, 0, 0] {
                    model_color_for_intensity(max_intensity, palette)
                } else {
                    best_color
                };
                frame.has_color = true;
            }
        }
    }
}

fn safe_neighbor_bit(bit: u8) -> u8 {
    match bit {
        0x01 => 0x02,
        0x02 => 0x04,
        0x04 => 0x40,
        0x08 => 0x10,
        0x10 => 0x20,
        0x20 => 0x80,
        0x40 => 0x04,
        0x80 => 0x20,
        _ => 0,
    }
}

pub fn update_safe_visibility_state(
    scratch: &mut RenderScratch,
    profile: BrailleProfile,
    ratio: f32,
) {
    if !matches!(profile, BrailleProfile::Safe) {
        scratch.safe_low_visibility_streak = 0;
        scratch.safe_high_visibility_streak = 0;
        scratch.safe_boost_active = false;
        return;
    }
    if ratio < 0.010 {
        scratch.safe_low_visibility_streak = scratch.safe_low_visibility_streak.saturating_add(1);
        scratch.safe_high_visibility_streak = 0;
        if scratch.safe_low_visibility_streak >= 8 {
            scratch.safe_boost_active = true;
        }
    } else if ratio > 0.020 {
        scratch.safe_high_visibility_streak = scratch.safe_high_visibility_streak.saturating_add(1);
        scratch.safe_low_visibility_streak = 0;
        if scratch.safe_high_visibility_streak >= 24 {
            scratch.safe_boost_active = false;
        }
    } else {
        scratch.safe_low_visibility_streak = 0;
        scratch.safe_high_visibility_streak = 0;
    }
}
