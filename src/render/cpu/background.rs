use crate::scene::{ColorMode, DetailProfile, RenderConfig, StageRole, ThemeStyle};

use crate::render::renderer::{FrameBuffers, ThemePalette};

#[derive(Debug, Clone, Copy)]
pub(crate) struct StageParams {
    pub bg_luma_scale: f32,
    pub floor_grid_density: usize,
    pub fog_mul: f32,
    pub backlight_strength: f32,
    pub particle_density: f32,
}

pub(crate) const BACKGROUND_ASCII: [char; 4] = [' ', ' ', '.', ':'];
pub(crate) const BACKGROUND_BRAILLE: [char; 3] = [' ', '⠈', '⠐'];

pub(crate) fn stage_params(config: &RenderConfig) -> StageParams {
    match config.stage_level.min(4) {
        0 => StageParams {
            bg_luma_scale: 0.28,
            floor_grid_density: 0,
            fog_mul: 1.25,
            backlight_strength: 0.12,
            particle_density: 0.00,
        },
        1 => StageParams {
            bg_luma_scale: 0.62,
            floor_grid_density: 18,
            fog_mul: 1.12,
            backlight_strength: 0.35,
            particle_density: 0.01,
        },
        2 => StageParams {
            bg_luma_scale: 1.0,
            floor_grid_density: 14,
            fog_mul: 1.0,
            backlight_strength: 0.50,
            particle_density: 0.02,
        },
        3 => StageParams {
            bg_luma_scale: 1.22,
            floor_grid_density: 10,
            fog_mul: 0.9,
            backlight_strength: 0.68,
            particle_density: 0.03,
        },
        _ => StageParams {
            bg_luma_scale: 1.40,
            floor_grid_density: 8,
            fog_mul: 0.82,
            backlight_strength: 0.84,
            particle_density: 0.04,
        },
    }
}

pub(crate) fn theme_palette(style: ThemeStyle) -> ThemePalette {
    match style {
        ThemeStyle::Theater => ThemePalette {
            shadow: [118, 106, 92],
            mid: [196, 160, 120],
            highlight: [255, 214, 152],
            bg: [38, 42, 54],
        },
        ThemeStyle::Neon => ThemePalette {
            shadow: [85, 145, 185],
            mid: [98, 214, 193],
            highlight: [243, 115, 210],
            bg: [18, 20, 34],
        },
        ThemeStyle::Holo => ThemePalette {
            shadow: [116, 170, 210],
            mid: [170, 220, 242],
            highlight: [236, 250, 255],
            bg: [20, 26, 38],
        },
    }
}

fn mix_color(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
}

fn background_detail_level(config: &RenderConfig) -> u8 {
    let mut level: u8 = if config.triangle_stride >= 3 || config.min_triangle_area_px2 >= 1.0 {
        2
    } else if config.triangle_stride >= 2 || config.min_triangle_area_px2 >= 0.5 {
        1
    } else {
        0
    };
    if matches!(config.detail_profile, DetailProfile::Perf) {
        level = (level + 1).min(2);
    } else if matches!(config.detail_profile, DetailProfile::Ultra) {
        level = level.saturating_sub(1);
    }
    level
}

fn hash01(x: usize, y: usize, salt: usize) -> f32 {
    let mut v = x
        .wrapping_mul(73856093)
        .wrapping_add(y.wrapping_mul(19349663))
        .wrapping_add(salt.wrapping_mul(83492791));
    v ^= v >> 13;
    v = v.wrapping_mul(0x5bd1e995);
    v ^= v >> 15;
    let folded = (v as u64 ^ ((v as u64) >> 32)) as u32;
    (folded as f32) / (u32::MAX as f32)
}

pub(crate) fn fill_background_ascii(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    palette: ThemePalette,
) {
    frame.depth.fill(f32::INFINITY);
    if frame.width == 0 || frame.height == 0 {
        frame.glyphs.clear();
        return;
    }
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let inv_w = if width > 1 {
        1.0 / ((width - 1) as f32)
    } else {
        1.0
    };
    let inv_h = if height > 1 {
        1.0 / ((height - 1) as f32)
    } else {
        1.0
    };
    let detail = background_detail_level(config);
    let stage = stage_params(config);
    let mut bg_attenuation = (1.0 - config.bg_suppression.clamp(0.0, 1.0) * 0.75).clamp(0.05, 1.0);
    let stage_cap = if matches!(config.stage_role, StageRole::Off) {
        0.0
    } else {
        config.stage_luma_cap.clamp(0.0, 1.0)
    };
    if matches!(config.stage_role, StageRole::Off) {
        bg_attenuation *= 0.55;
    }
    let pulse_scale = if matches!(config.color_mode, ColorMode::Ansi) {
        1.0
    } else if config.stage_reactive {
        1.0 + config.reactive_pulse * 0.25
    } else {
        1.0
    };
    frame.has_color = matches!(config.color_mode, ColorMode::Ansi);
    for y in 0..height {
        let v = (y as f32) * inv_h;
        let horizon = (1.0 - v).powf(1.35);
        for x in 0..width {
            let u = (x as f32) * inv_w;
            let nx = u * 2.0 - 1.0;
            let ny = v * 2.0 - 1.0;
            let vignette = (1.0 - (nx * nx + ny * ny).sqrt() * 0.85).clamp(0.0, 1.0);
            let mut intensity = match detail {
                0 => ((0.02 + horizon * 0.06 + vignette * 0.07) * pulse_scale).clamp(0.0, 0.22),
                1 => ((0.015 + horizon * 0.05 + vignette * 0.03) * pulse_scale).clamp(0.0, 0.16),
                _ => (0.010 + horizon * 0.025).clamp(0.0, 0.11),
            };
            intensity *= stage.bg_luma_scale * bg_attenuation * stage_cap;
            if stage_cap > 0.01
                && stage.floor_grid_density > 0
                && y > (height * 2) / 3
                && (x % stage.floor_grid_density == 0)
            {
                intensity += 0.05 * stage.backlight_strength;
            }
            if stage_cap > 0.01
                && stage.particle_density > 0.0
                && hash01(x, y, 17) < stage.particle_density
            {
                intensity += 0.07 * stage.backlight_strength;
            }
            let index = ((intensity * (BACKGROUND_ASCII.len() as f32 - 1.0)).round() as usize)
                .min(BACKGROUND_ASCII.len() - 1);
            let dst = y * width + x;
            frame.glyphs[dst] = if matches!(config.color_mode, ColorMode::Ansi) || detail >= 2 {
                ' '
            } else {
                BACKGROUND_ASCII[index]
            };
            frame.fg_rgb[dst] = mix_color(
                palette.bg,
                palette.shadow,
                if matches!(config.color_mode, ColorMode::Ansi) {
                    0.18
                } else {
                    intensity * 0.4
                },
            );
        }
    }
}

pub(crate) fn fill_background_braille(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    palette: ThemePalette,
) {
    frame.depth.fill(f32::INFINITY);
    if frame.width == 0 || frame.height == 0 {
        frame.glyphs.clear();
        return;
    }
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let inv_w = if width > 1 {
        1.0 / ((width - 1) as f32)
    } else {
        1.0
    };
    let inv_h = if height > 1 {
        1.0 / ((height - 1) as f32)
    } else {
        1.0
    };
    let detail = background_detail_level(config);
    let stage = stage_params(config);
    let mut bg_attenuation = (1.0 - config.bg_suppression.clamp(0.0, 1.0) * 0.75).clamp(0.05, 1.0);
    let stage_cap = if matches!(config.stage_role, StageRole::Off) {
        0.0
    } else {
        config.stage_luma_cap.clamp(0.0, 1.0)
    };
    if matches!(config.stage_role, StageRole::Off) {
        bg_attenuation *= 0.55;
    }
    let pulse_scale = if matches!(config.color_mode, ColorMode::Ansi) {
        1.0
    } else if config.stage_reactive {
        1.0 + config.reactive_pulse * 0.22
    } else {
        1.0
    };
    frame.has_color = matches!(config.color_mode, ColorMode::Ansi);
    for y in 0..height {
        let v = (y as f32) * inv_h;
        let horizon = (1.0 - v).powf(1.2);
        for x in 0..width {
            let u = (x as f32) * inv_w;
            let wave = if detail == 0 {
                ((u * 6.0 + v * 3.0).sin() * 0.5 + 0.5) * 0.03
            } else {
                0.0
            };
            let mut base = match detail {
                0 => ((0.015 + horizon * 0.05 + wave) * pulse_scale).clamp(0.0, 0.18),
                1 => ((0.011 + horizon * 0.035) * pulse_scale).clamp(0.0, 0.12),
                _ => (0.008 + horizon * 0.015).clamp(0.0, 0.08),
            };
            base *= stage.bg_luma_scale * bg_attenuation * stage_cap;
            if stage_cap > 0.01
                && stage.floor_grid_density > 0
                && y > (height * 2) / 3
                && (x % stage.floor_grid_density == 0)
            {
                base += 0.04 * stage.backlight_strength;
            }
            if stage_cap > 0.01
                && stage.particle_density > 0.0
                && hash01(x, y, 29) < stage.particle_density
            {
                base += 0.05 * stage.backlight_strength;
            }
            let index = ((base * (BACKGROUND_BRAILLE.len() as f32 - 1.0)).round() as usize)
                .min(BACKGROUND_BRAILLE.len() - 1);
            let dst = y * width + x;
            frame.glyphs[dst] = if matches!(config.color_mode, ColorMode::Ansi) || detail >= 2 {
                ' '
            } else {
                BACKGROUND_BRAILLE[index]
            };
            frame.fg_rgb[dst] = mix_color(
                palette.bg,
                palette.shadow,
                if matches!(config.color_mode, ColorMode::Ansi) {
                    0.18
                } else {
                    base * 0.5
                },
            );
        }
    }
}
