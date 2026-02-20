use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use std::fmt::Write as _;

use crate::math::{depth_less, perspective_matrix};
use crate::scene::{
    BrailleProfile, ColorMode, ContrastProfile, DEFAULT_CHARSET, DetailProfile, MeshCpu,
    RenderConfig, RenderMode, SceneCpu, TextureSamplingMode, ThemeStyle,
};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 1.2, 4.0),
            target: Vec3::new(0.0, 1.0, 0.0),
            up: Vec3::Y,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameBuffers {
    pub width: u16,
    pub height: u16,
    pub glyphs: Vec<char>,
    pub depth: Vec<f32>,
    pub fg_rgb: Vec<[u8; 3]>,
    pub has_color: bool,
}

impl FrameBuffers {
    pub fn new(width: u16, height: u16) -> Self {
        let size = usize::from(width).saturating_mul(usize::from(height));
        Self {
            width,
            height,
            glyphs: vec![' '; size],
            depth: vec![f32::INFINITY; size],
            fg_rgb: vec![[255, 255, 255]; size],
            has_color: false,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width).saturating_mul(usize::from(height));
        self.glyphs.resize(size, ' ');
        self.depth.resize(size, f32::INFINITY);
        self.fg_rgb.resize(size, [255, 255, 255]);
        self.has_color = false;
    }

    pub fn clear(&mut self, glyph: char) {
        self.glyphs.fill(glyph);
        self.depth.fill(f32::INFINITY);
        self.fg_rgb.fill([255, 255, 255]);
        self.has_color = false;
    }

    pub fn as_text(&self) -> String {
        let mut out = String::new();
        self.write_text(&mut out);
        out
    }

    pub fn write_text(&self, out: &mut String) {
        out.clear();
        out.reserve(
            self.glyphs
                .len()
                .saturating_add(usize::from(self.height).saturating_sub(1)),
        );
        let width = usize::from(self.width);
        for y in 0..usize::from(self.height) {
            let row_start = y * width;
            let row_end = row_start + width;
            for c in &self.glyphs[row_start..row_end] {
                out.push(*c);
            }
            if y + 1 < usize::from(self.height) {
                out.push('\n');
            }
        }
    }

    pub fn write_ansi_text(&self, out: &mut String) {
        if !self.has_color {
            self.write_text(out);
            return;
        }
        out.clear();
        out.reserve(
            self.glyphs
                .len()
                .saturating_mul(20)
                .saturating_add(usize::from(self.height).saturating_mul(4)),
        );
        let width = usize::from(self.width);
        for y in 0..usize::from(self.height) {
            let row_start = y * width;
            let row_end = row_start + width;
            let mut current_rgb: Option<[u8; 3]> = None;
            for idx in row_start..row_end {
                let rgb = quantize_rgb16(self.fg_rgb[idx]);
                if current_rgb != Some(rgb) {
                    push_fg_ansi(out, rgb);
                    current_rgb = Some(rgb);
                }
                out.push(self.glyphs[idx]);
            }
            out.push_str("\x1b[0m");
            if y + 1 < usize::from(self.height) {
                out.push('\n');
            }
        }
    }
}

fn push_fg_ansi(out: &mut String, rgb: [u8; 3]) {
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

#[derive(Debug, Clone, Copy)]
pub struct ColorCell {
    pub ch: char,
    pub fg_rgb: [u8; 3],
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RenderStats {
    pub triangles_total: usize,
    pub triangles_culled: usize,
    pub pixels_drawn: usize,
    pub visible_cell_ratio: f32,
    pub visible_centroid_px: Option<(f32, f32)>,
    pub root_screen_px: Option<(f32, f32)>,
    pub root_depth: Option<f32>,
    pub visible_bbox_px: Option<(u16, u16, u16, u16)>,
    pub visible_bbox_aspect: f32,
    pub visible_height_ratio: f32,
}

const BACKGROUND_ASCII: [char; 4] = [' ', ' ', '.', ':'];
const BACKGROUND_BRAILLE: [char; 3] = [' ', '⠈', '⠐'];

#[derive(Debug, Clone, Copy)]
struct ThemePalette {
    shadow: [u8; 3],
    mid: [u8; 3],
    highlight: [u8; 3],
    bg: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
struct StageParams {
    bg_luma_scale: f32,
    floor_grid_density: usize,
    fog_mul: f32,
    backlight_strength: f32,
    particle_density: f32,
}

fn stage_params(config: &RenderConfig) -> StageParams {
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

fn theme_palette(style: ThemeStyle) -> ThemePalette {
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

fn model_color_for_intensity(intensity: f32, palette: ThemePalette) -> [u8; 3] {
    let t = intensity.clamp(0.0, 1.0);
    if t < 0.58 {
        mix_color(palette.shadow, palette.mid, t / 0.58)
    } else {
        mix_color(palette.mid, palette.highlight, (t - 0.58) / 0.42)
    }
}

fn fill_background_ascii(frame: &mut FrameBuffers, config: &RenderConfig, palette: ThemePalette) {
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
            intensity *= stage.bg_luma_scale;
            if stage.floor_grid_density > 0
                && y > (height * 2) / 3
                && (x % stage.floor_grid_density == 0)
            {
                intensity += 0.05 * stage.backlight_strength;
            }
            if stage.particle_density > 0.0 && hash01(x, y, 17) < stage.particle_density {
                intensity += 0.07 * stage.backlight_strength;
            }
            let index = ((intensity * (BACKGROUND_ASCII.len() as f32 - 1.0)).round() as usize)
                .min(BACKGROUND_ASCII.len() - 1);
            let dst = y * width + x;
            frame.glyphs[dst] = if matches!(config.color_mode, ColorMode::Ansi) {
                ' '
            } else if detail >= 2 {
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

fn fill_background_braille(frame: &mut FrameBuffers, config: &RenderConfig, palette: ThemePalette) {
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
            base *= stage.bg_luma_scale;
            if stage.floor_grid_density > 0
                && y > (height * 2) / 3
                && (x % stage.floor_grid_density == 0)
            {
                base += 0.04 * stage.backlight_strength;
            }
            if stage.particle_density > 0.0 && hash01(x, y, 29) < stage.particle_density {
                base += 0.05 * stage.backlight_strength;
            }
            let index = ((base * (BACKGROUND_BRAILLE.len() as f32 - 1.0)).round() as usize)
                .min(BACKGROUND_BRAILLE.len() - 1);
            let dst = y * width + x;
            frame.glyphs[dst] = if matches!(config.color_mode, ColorMode::Ansi) {
                ' '
            } else if detail >= 2 {
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

fn sample_material_albedo(
    scene: &SceneCpu,
    material_index: Option<usize>,
    uv0: Vec2,
    vertex_color: [f32; 4],
    config: &RenderConfig,
) -> [f32; 3] {
    if !config.material_color {
        return [1.0, 1.0, 1.0];
    }
    let mut color = [
        vertex_color[0],
        vertex_color[1],
        vertex_color[2],
        vertex_color[3],
    ];
    if let Some(material) = material_index.and_then(|index| scene.materials.get(index)) {
        color[0] *= material.base_color_factor[0];
        color[1] *= material.base_color_factor[1];
        color[2] *= material.base_color_factor[2];
        color[3] *= material.base_color_factor[3];
        if let Some(texture_index) = material.base_color_texture {
            if let Some(texture) = scene.textures.get(texture_index) {
                let sampled = sample_texture_rgba(texture, uv0, config.texture_sampling);
                color[0] *= sampled[0];
                color[1] *= sampled[1];
                color[2] *= sampled[2];
                color[3] *= sampled[3];
            }
        }
    }
    [
        color[0].clamp(0.0, 1.0),
        color[1].clamp(0.0, 1.0),
        color[2].clamp(0.0, 1.0),
    ]
}

fn sample_texture_rgba(
    texture: &crate::scene::TextureCpu,
    uv0: Vec2,
    mode: TextureSamplingMode,
) -> [f32; 4] {
    let wrap_u = uv0.x - uv0.x.floor();
    let wrap_v = uv0.y - uv0.y.floor();
    match mode {
        TextureSamplingMode::Nearest => sample_texture_nearest(texture, wrap_u, wrap_v),
        TextureSamplingMode::Bilinear => sample_texture_bilinear(texture, wrap_u, wrap_v),
    }
}

fn sample_texture_nearest(texture: &crate::scene::TextureCpu, u: f32, v: f32) -> [f32; 4] {
    if texture.width == 0 || texture.height == 0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let x = ((u * texture.width as f32).floor() as i32).rem_euclid(texture.width as i32) as u32;
    let y = (((1.0 - v) * texture.height as f32).floor() as i32).rem_euclid(texture.height as i32)
        as u32;
    sample_texture_texel(texture, x, y)
}

fn sample_texture_bilinear(texture: &crate::scene::TextureCpu, u: f32, v: f32) -> [f32; 4] {
    if texture.width == 0 || texture.height == 0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let fx = u * texture.width as f32 - 0.5;
    let fy = (1.0 - v) * texture.height as f32 - 0.5;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let c00 = sample_texture_texel(
        texture,
        x0.rem_euclid(texture.width as i32) as u32,
        y0.rem_euclid(texture.height as i32) as u32,
    );
    let c10 = sample_texture_texel(
        texture,
        x1.rem_euclid(texture.width as i32) as u32,
        y0.rem_euclid(texture.height as i32) as u32,
    );
    let c01 = sample_texture_texel(
        texture,
        x0.rem_euclid(texture.width as i32) as u32,
        y1.rem_euclid(texture.height as i32) as u32,
    );
    let c11 = sample_texture_texel(
        texture,
        x1.rem_euclid(texture.width as i32) as u32,
        y1.rem_euclid(texture.height as i32) as u32,
    );
    [
        bilerp(c00[0], c10[0], c01[0], c11[0], tx, ty),
        bilerp(c00[1], c10[1], c01[1], c11[1], tx, ty),
        bilerp(c00[2], c10[2], c01[2], c11[2], tx, ty),
        bilerp(c00[3], c10[3], c01[3], c11[3], tx, ty),
    ]
}

fn sample_texture_texel(texture: &crate::scene::TextureCpu, x: u32, y: u32) -> [f32; 4] {
    let idx = (y as usize)
        .saturating_mul(texture.width as usize)
        .saturating_add(x as usize)
        .saturating_mul(4);
    if idx + 3 >= texture.rgba8.len() {
        return [1.0, 1.0, 1.0, 1.0];
    }
    [
        texture.rgba8[idx] as f32 / 255.0,
        texture.rgba8[idx + 1] as f32 / 255.0,
        texture.rgba8[idx + 2] as f32 / 255.0,
        texture.rgba8[idx + 3] as f32 / 255.0,
    ]
}

fn bilerp(c00: f32, c10: f32, c01: f32, c11: f32, tx: f32, ty: f32) -> f32 {
    let a = c00 + (c10 - c00) * tx;
    let b = c01 + (c11 - c01) * tx;
    a + (b - a) * ty
}

fn luminance(rgb: [f32; 3]) -> f32 {
    (rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722).clamp(0.0, 1.0)
}

fn scale_rgb(rgb: [f32; 3], scale: f32) -> [f32; 3] {
    [
        (rgb[0] * scale).clamp(0.0, 1.0),
        (rgb[1] * scale).clamp(0.0, 1.0),
        (rgb[2] * scale).clamp(0.0, 1.0),
    ]
}

fn to_display_rgb(rgb: [f32; 3]) -> [u8; 3] {
    [
        (rgb[0].powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8,
        (rgb[1].powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8,
        (rgb[2].powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn project_root_screen(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    model_rotation: Mat4,
    view_projection: Mat4,
    width: u16,
    height: u16,
) -> Option<(f32, f32, f32)> {
    let node_index = scene.root_center_node?;
    let global = global_matrices
        .get(node_index)
        .copied()
        .unwrap_or(Mat4::IDENTITY);
    let world = (model_rotation * global).transform_point3(Vec3::ZERO);
    let clip = view_projection * world.extend(1.0);
    if clip.w <= 1e-5 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    let x = (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0);
    let y = (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0);
    let depth = (ndc.z + 1.0) * 0.5;
    Some((x, y, depth))
}

#[derive(Debug, Clone)]
pub struct GlyphRamp {
    chars: Vec<char>,
}

impl GlyphRamp {
    pub fn from_config(config: &RenderConfig) -> Self {
        let source = if config.mode == RenderMode::Braille {
            BRAILLE_RAMP
        } else if config.charset.is_empty() {
            " "
        } else {
            config.charset.as_str()
        };
        let mut chars: Vec<char> = source.chars().collect();
        if chars.is_empty() {
            chars.push(' ');
        }
        Self { chars }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }
}

#[derive(Debug, Default)]
pub struct RenderScratch {
    projected_vertices: Vec<Option<ProjectedVertex>>,
    braille_subpixels: BrailleSubpixelBuffers,
    exposure: f32,
    safe_low_visibility_streak: u32,
    safe_high_visibility_streak: u32,
    safe_boost_active: bool,
}

impl RenderScratch {
    pub fn with_capacity(vertex_capacity: usize) -> Self {
        Self {
            projected_vertices: Vec::with_capacity(vertex_capacity),
            braille_subpixels: BrailleSubpixelBuffers::default(),
            exposure: 1.0,
            safe_low_visibility_streak: 0,
            safe_high_visibility_streak: 0,
            safe_boost_active: false,
        }
    }

    fn prepare_projected_vertices(
        &mut self,
        vertex_count: usize,
    ) -> &mut [Option<ProjectedVertex>] {
        self.projected_vertices.clear();
        self.projected_vertices.resize(vertex_count, None);
        self.projected_vertices.as_mut_slice()
    }
}

#[derive(Debug, Default, Clone)]
pub struct BrailleSubpixelBuffers {
    pub width: u16,
    pub height: u16,
    pub depth: Vec<f32>,
    pub intensity: Vec<f32>,
    pub color_rgb: Vec<[u8; 3]>,
}

impl BrailleSubpixelBuffers {
    fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width).saturating_mul(usize::from(height));
        self.depth.resize(size, f32::INFINITY);
        self.intensity.resize(size, 0.0);
        self.color_rgb.resize(size, [0, 0, 0]);
    }

    fn clear(&mut self) {
        self.depth.fill(f32::INFINITY);
        self.intensity.fill(0.0);
        self.color_rgb.fill([0, 0, 0]);
    }
}

#[derive(Debug, Clone, Copy)]
struct ProjectedVertex {
    screen: Vec2,
    depth: f32,
    world_pos: Vec3,
    world_normal: Vec3,
    uv0: Vec2,
    vertex_color: [f32; 4],
    material_index: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct ShadingParams {
    light_dir: Vec3,
    camera_pos: Vec3,
    ambient: f32,
    diffuse_strength: f32,
    specular_strength: f32,
    specular_power: f32,
    rim_strength: f32,
    rim_power: f32,
    fog_strength: f32,
}

#[derive(Debug, Clone, Copy)]
struct ContrastParams {
    floor: f32,
    gamma: f32,
    fog_scale: f32,
}

pub fn render_frame(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    match config.mode {
        RenderMode::Ascii => render_frame_ascii(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ),
        RenderMode::Braille => render_frame_braille(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            scratch,
            camera,
            model_rotation_y,
        ),
    }
}

fn render_frame_ascii(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let palette = theme_palette(config.theme_style);
    fill_background_ascii(frame, config, palette);
    if frame.width == 0 || frame.height == 0 {
        return RenderStats::default();
    }
    let cells = usize::from(frame.width).saturating_mul(usize::from(frame.height));
    let contrast = contrast_params(config, cells);
    let charset = select_charset(config, glyph_ramp.chars(), cells);
    let stage = stage_params(config);
    let aspect = ((frame.width as f32) * config.cell_aspect).max(1.0) / (frame.height as f32);
    let projection = perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;
    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    let shading = ShadingParams {
        light_dir: Vec3::new(0.3, 0.7, 0.6).normalize(),
        camera_pos: camera.eye,
        ambient: config.ambient.max(0.0),
        diffuse_strength: config.diffuse_strength.max(0.0),
        specular_strength: config.specular_strength.max(0.0),
        specular_power: config.specular_power.max(1.0),
        rim_strength: config.rim_strength.max(0.0),
        rim_power: config.rim_power.max(0.01),
        fog_strength: (config.fog_strength * stage.fog_mul).clamp(0.0, 1.0),
    };
    let mut stats = RenderStats::default();
    if let Some((x, y, depth)) = project_root_screen(
        scene,
        global_matrices,
        model_rotation,
        view_projection,
        frame.width,
        frame.height,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }
    let exposure = scratch.exposure * exposure_bias_multiplier(config.exposure_bias);
    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let node_global = global_matrices
            .get(instance.node_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        let model = model_rotation * node_global;
        let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
        {
            let projected_vertices = scratch.prepare_projected_vertices(mesh.positions.len());
            project_mesh_vertices(
                mesh,
                model,
                normal_matrix,
                view_projection,
                frame.width,
                frame.height,
                instance.skin_index.and_then(|i| skin_matrices.get(i)),
                instance_morph_weights
                    .get(instance_index)
                    .map(Vec::as_slice),
                projected_vertices,
            );
        }
        let projected_vertices = scratch.projected_vertices.as_slice();
        stats.triangles_total += mesh.indices.len();
        rasterize_mesh(
            mesh,
            projected_vertices,
            frame,
            charset,
            config,
            scene,
            &mut stats,
            shading,
            contrast,
            palette,
            exposure,
        );
    }
    apply_visible_metrics(&mut stats, frame);
    stats
}

fn render_frame_braille(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let palette = theme_palette(config.theme_style);
    fill_background_braille(frame, config, palette);
    if frame.width == 0 || frame.height == 0 {
        return RenderStats::default();
    }
    let sub_w = frame.width.saturating_mul(2).max(1);
    let sub_h = frame.height.saturating_mul(4).max(1);
    scratch.braille_subpixels.resize(sub_w, sub_h);
    scratch.braille_subpixels.clear();

    let cells = usize::from(frame.width).saturating_mul(usize::from(frame.height));
    let contrast = contrast_params(config, cells);
    let stage = stage_params(config);
    let aspect = ((sub_w as f32)
        * (config.cell_aspect * 2.0 * config.braille_aspect_compensation.clamp(0.70, 1.30)))
    .max(1.0)
        / (sub_h as f32);
    let projection = perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;
    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    let shading = ShadingParams {
        light_dir: Vec3::new(0.3, 0.7, 0.6).normalize(),
        camera_pos: camera.eye,
        ambient: config.ambient.max(0.0),
        diffuse_strength: config.diffuse_strength.max(0.0),
        specular_strength: config.specular_strength.max(0.0),
        specular_power: config.specular_power.max(1.0),
        rim_strength: config.rim_strength.max(0.0),
        rim_power: config.rim_power.max(0.01),
        fog_strength: (config.fog_strength * stage.fog_mul).clamp(0.0, 1.0),
    };

    let mut histogram = [0_u32; 64];
    let mut histogram_count = 0_u32;
    let mut stats = RenderStats::default();
    if let Some((x, y, depth)) = project_root_screen(
        scene,
        global_matrices,
        model_rotation,
        view_projection,
        frame.width,
        frame.height,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }
    let exposure = scratch.exposure * exposure_bias_multiplier(config.exposure_bias);
    let threshold = braille_thresholds(config.braille_profile, scratch.safe_boost_active);
    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let node_global = global_matrices
            .get(instance.node_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        let model = model_rotation * node_global;
        let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
        {
            let projected_vertices = scratch.prepare_projected_vertices(mesh.positions.len());
            project_mesh_vertices(
                mesh,
                model,
                normal_matrix,
                view_projection,
                sub_w,
                sub_h,
                instance.skin_index.and_then(|i| skin_matrices.get(i)),
                instance_morph_weights
                    .get(instance_index)
                    .map(Vec::as_slice),
                projected_vertices,
            );
        }
        let projected_vertices = scratch.projected_vertices.as_slice();
        let subpixels = &mut scratch.braille_subpixels;
        stats.triangles_total += mesh.indices.len();
        rasterize_braille_mesh(
            mesh,
            projected_vertices,
            subpixels,
            config,
            scene,
            &mut stats,
            shading,
            contrast,
            palette,
            exposure,
            threshold,
            &mut histogram,
            &mut histogram_count,
        );
    }
    update_exposure_from_histogram(
        &mut scratch.exposure,
        &histogram,
        histogram_count,
        config.braille_profile,
    );
    compose_braille_cells(
        frame,
        &scratch.braille_subpixels,
        config,
        palette,
        threshold,
    );
    apply_visible_metrics(&mut stats, frame);
    update_safe_visibility_state(scratch, config.braille_profile, stats.visible_cell_ratio);
    stats
}

pub fn encode_ansi_frame(frame: &FrameBuffers, out: &mut String) {
    frame.write_ansi_text(out);
}

fn project_mesh_vertices(
    mesh: &MeshCpu,
    model: Mat4,
    normal_matrix: Mat3,
    view_projection: Mat4,
    width: u16,
    height: u16,
    skin_matrices: Option<&Vec<Mat4>>,
    morph_weights: Option<&[f32]>,
    projected_vertices: &mut [Option<ProjectedVertex>],
) {
    for (index, position) in mesh.positions.iter().enumerate() {
        let base_normal = mesh
            .normals
            .get(index)
            .copied()
            .unwrap_or(Vec3::new(0.0, 1.0, 0.0));
        let (morphed_pos, morphed_normal) =
            apply_morph_targets(mesh, index, *position, base_normal, morph_weights);
        let (skinned_pos, skinned_normal) =
            apply_skin(mesh, index, morphed_pos, morphed_normal, skin_matrices);
        let world_pos = model.transform_point3(skinned_pos);
        let world_normal = (normal_matrix * skinned_normal).normalize_or_zero();
        let clip = view_projection * world_pos.extend(1.0);
        if clip.w <= 1e-5 {
            projected_vertices[index] = None;
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 {
            projected_vertices[index] = None;
            continue;
        }
        let screen = Vec2::new(
            (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0),
            (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0),
        );
        let depth = (ndc.z + 1.0) * 0.5;
        let uv0 = mesh
            .uv0
            .as_ref()
            .and_then(|values| values.get(index).copied())
            .unwrap_or(Vec2::ZERO);
        let vertex_color = mesh
            .colors_rgba
            .as_ref()
            .and_then(|values| values.get(index).copied())
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        projected_vertices[index] = Some(ProjectedVertex {
            screen,
            depth,
            world_pos,
            world_normal,
            uv0,
            vertex_color,
            material_index: mesh.material_index,
        });
    }
}

fn apply_skin(
    mesh: &MeshCpu,
    vertex_index: usize,
    position: Vec3,
    normal: Vec3,
    skin_matrices: Option<&Vec<Mat4>>,
) -> (Vec3, Vec3) {
    let Some(joints) = mesh.joints4.as_ref() else {
        return (position, normal);
    };
    let Some(weights) = mesh.weights4.as_ref() else {
        return (position, normal);
    };
    let Some(skin_matrices) = skin_matrices else {
        return (position, normal);
    };

    let joints = match joints.get(vertex_index) {
        Some(value) => value,
        None => return (position, normal),
    };
    let weights = match weights.get(vertex_index) {
        Some(value) => value,
        None => return (position, normal),
    };

    let mut skinned_pos = Vec4::ZERO;
    let mut skinned_nrm = Vec3::ZERO;
    let mut accumulated = 0.0;
    for i in 0..4 {
        let weight = weights[i];
        if weight <= 0.0 {
            continue;
        }
        let joint_idx = joints[i] as usize;
        let Some(joint_matrix) = skin_matrices.get(joint_idx) else {
            continue;
        };
        skinned_pos += (*joint_matrix * position.extend(1.0)) * weight;
        skinned_nrm += (Mat3::from_mat4(*joint_matrix) * normal) * weight;
        accumulated += weight;
    }
    if accumulated <= f32::EPSILON {
        return (position, normal);
    }
    let out_pos = if skinned_pos.w.abs() > 1e-6 {
        skinned_pos.truncate() / skinned_pos.w
    } else {
        skinned_pos.truncate()
    };
    (out_pos, skinned_nrm.normalize_or_zero())
}

fn apply_morph_targets(
    mesh: &MeshCpu,
    vertex_index: usize,
    base_position: Vec3,
    base_normal: Vec3,
    morph_weights: Option<&[f32]>,
) -> (Vec3, Vec3) {
    let Some(weights) = morph_weights else {
        return (base_position, base_normal);
    };
    if mesh.morph_targets.is_empty() || weights.is_empty() {
        return (base_position, base_normal);
    }

    let mut out_position = base_position;
    let mut out_normal = base_normal;
    for (target_index, target) in mesh.morph_targets.iter().enumerate() {
        let weight = weights.get(target_index).copied().unwrap_or(0.0);
        if weight.abs() <= 1e-5 {
            continue;
        }
        if let Some(delta) = target.position_deltas.get(vertex_index) {
            out_position += *delta * weight;
        }
        if let Some(delta) = target.normal_deltas.get(vertex_index) {
            out_normal += *delta * weight;
        }
    }
    (out_position, out_normal.normalize_or_zero())
}

fn rasterize_mesh(
    mesh: &MeshCpu,
    projected_vertices: &[Option<ProjectedVertex>],
    frame: &mut FrameBuffers,
    charset: &[char],
    config: &RenderConfig,
    scene: &SceneCpu,
    stats: &mut RenderStats,
    shading: ShadingParams,
    contrast: ContrastParams,
    palette: ThemePalette,
    exposure: f32,
) {
    let width = i32::from(frame.width);
    let height = i32::from(frame.height);
    let width_usize = usize::from(frame.width);
    let triangle_stride = config.triangle_stride.max(1);
    let min_triangle_area_px2 = config.min_triangle_area_px2.max(0.0);
    if width <= 0 || height <= 0 {
        return;
    }

    for (triangle_index, tri) in mesh.indices.iter().enumerate() {
        if triangle_stride > 1 && (triangle_index % triangle_stride) != 0 {
            continue;
        }
        let (Some(v0), Some(v1), Some(v2)) = (
            projected_vertices.get(tri[0] as usize).copied().flatten(),
            projected_vertices.get(tri[1] as usize).copied().flatten(),
            projected_vertices.get(tri[2] as usize).copied().flatten(),
        ) else {
            continue;
        };

        let signed_area = perp_dot(v1.screen - v0.screen, v2.screen - v0.screen);
        if signed_area.abs() < 1e-8 || signed_area.abs() < min_triangle_area_px2 {
            stats.triangles_culled += 1;
            continue;
        }
        // edge(a,b,p) uses (p-a)x(b-a), so it has opposite sign from perp_dot((b-a),(p-a)).
        let inv_area = -1.0 / signed_area;

        let min_x = v0
            .screen
            .x
            .min(v1.screen.x.min(v2.screen.x))
            .floor()
            .max(0.0) as i32;
        let max_x = v0
            .screen
            .x
            .max(v1.screen.x.max(v2.screen.x))
            .ceil()
            .min((width - 1) as f32) as i32;
        let min_y = v0
            .screen
            .y
            .min(v1.screen.y.min(v2.screen.y))
            .floor()
            .max(0.0) as i32;
        let max_y = v0
            .screen
            .y
            .max(v1.screen.y.max(v2.screen.y))
            .ceil()
            .min((height - 1) as f32) as i32;

        if min_x > max_x || min_y > max_y {
            continue;
        }

        let edge0_a = v2.screen.y - v1.screen.y;
        let edge0_b = v1.screen.x - v2.screen.x;
        let edge0_c = v2.screen.x * v1.screen.y - v1.screen.x * v2.screen.y;
        let edge1_a = v0.screen.y - v2.screen.y;
        let edge1_b = v2.screen.x - v0.screen.x;
        let edge1_c = v0.screen.x * v2.screen.y - v2.screen.x * v0.screen.y;
        let edge2_a = v1.screen.y - v0.screen.y;
        let edge2_b = v0.screen.x - v1.screen.x;
        let edge2_c = v1.screen.x * v0.screen.y - v0.screen.x * v1.screen.y;

        let start_x = min_x as f32 + 0.5;
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let mut edge0 = edge0_a * start_x + edge0_b * py + edge0_c;
            let mut edge1 = edge1_a * start_x + edge1_b * py + edge1_c;
            let mut edge2 = edge2_a * start_x + edge2_b * py + edge2_c;
            for x in min_x..=max_x {
                let w0 = edge0 * inv_area;
                let w1 = edge1 * inv_area;
                let w2 = edge2 * inv_area;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }

                let depth = v0.depth * w0 + v1.depth * w1 + v2.depth * w2;
                if !(0.0..=1.0).contains(&depth) {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }
                let idx = (y as usize) * width_usize + (x as usize);
                if depth_less(frame.depth[idx], depth) {
                    frame.depth[idx] = depth;
                    let world_pos = v0.world_pos * w0 + v1.world_pos * w1 + v2.world_pos * w2;
                    let world_normal =
                        (v0.world_normal * w0 + v1.world_normal * w1 + v2.world_normal * w2)
                            .normalize_or_zero();
                    let uv0 = v0.uv0 * w0 + v1.uv0 * w1 + v2.uv0 * w2;
                    let vertex_color = [
                        v0.vertex_color[0] * w0 + v1.vertex_color[0] * w1 + v2.vertex_color[0] * w2,
                        v0.vertex_color[1] * w0 + v1.vertex_color[1] * w1 + v2.vertex_color[1] * w2,
                        v0.vertex_color[2] * w0 + v1.vertex_color[2] * w1 + v2.vertex_color[2] * w2,
                        v0.vertex_color[3] * w0 + v1.vertex_color[3] * w1 + v2.vertex_color[3] * w2,
                    ];
                    let material_index = v0
                        .material_index
                        .or(v1.material_index)
                        .or(v2.material_index);
                    let albedo =
                        sample_material_albedo(scene, material_index, uv0, vertex_color, config);
                    let lighting = shade_lighting(world_normal, world_pos, shading).clamp(0.0, 1.0);
                    let fog = depth.powf(1.7) * shading.fog_strength * contrast.fog_scale;
                    let base_light = (lighting * (1.0 - fog.clamp(0.0, 1.0))).clamp(0.0, 1.0);
                    let shaded_rgb = [
                        albedo[0] * base_light,
                        albedo[1] * base_light,
                        albedo[2] * base_light,
                    ];
                    let base = luminance(shaded_rgb);
                    let intensity =
                        tone_map_intensity(base, contrast.floor, contrast.gamma, exposure);
                    frame.glyphs[idx] = glyph_for_intensity(intensity, charset);
                    if matches!(config.color_mode, ColorMode::Ansi) {
                        frame.fg_rgb[idx] = if config.material_color {
                            to_display_rgb(scale_rgb(shaded_rgb, intensity.max(0.15)))
                        } else {
                            model_color_for_intensity(intensity, palette)
                        };
                        frame.has_color = true;
                    }
                    stats.pixels_drawn += 1;
                }
                edge0 += edge0_a;
                edge1 += edge1_a;
                edge2 += edge2_a;
            }
        }
    }
}

fn rasterize_braille_mesh(
    mesh: &MeshCpu,
    projected_vertices: &[Option<ProjectedVertex>],
    subpixels: &mut BrailleSubpixelBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    stats: &mut RenderStats,
    shading: ShadingParams,
    contrast: ContrastParams,
    palette: ThemePalette,
    exposure: f32,
    threshold: BrailleThresholds,
    histogram: &mut [u32; 64],
    histogram_count: &mut u32,
) {
    let width = i32::from(subpixels.width);
    let height = i32::from(subpixels.height);
    let width_usize = usize::from(subpixels.width);
    let triangle_stride = config.triangle_stride.max(1);
    let min_triangle_area_px2 = config.min_triangle_area_px2.max(0.0);
    if width <= 0 || height <= 0 {
        return;
    }

    for (triangle_index, tri) in mesh.indices.iter().enumerate() {
        if triangle_stride > 1 && (triangle_index % triangle_stride) != 0 {
            continue;
        }
        let (Some(v0), Some(v1), Some(v2)) = (
            projected_vertices.get(tri[0] as usize).copied().flatten(),
            projected_vertices.get(tri[1] as usize).copied().flatten(),
            projected_vertices.get(tri[2] as usize).copied().flatten(),
        ) else {
            continue;
        };

        let signed_area = perp_dot(v1.screen - v0.screen, v2.screen - v0.screen);
        if signed_area.abs() < 1e-8 || signed_area.abs() < min_triangle_area_px2 {
            stats.triangles_culled += 1;
            continue;
        }
        let inv_area = -1.0 / signed_area;
        let min_x = v0
            .screen
            .x
            .min(v1.screen.x.min(v2.screen.x))
            .floor()
            .max(0.0) as i32;
        let max_x = v0
            .screen
            .x
            .max(v1.screen.x.max(v2.screen.x))
            .ceil()
            .min((width - 1) as f32) as i32;
        let min_y = v0
            .screen
            .y
            .min(v1.screen.y.min(v2.screen.y))
            .floor()
            .max(0.0) as i32;
        let max_y = v0
            .screen
            .y
            .max(v1.screen.y.max(v2.screen.y))
            .ceil()
            .min((height - 1) as f32) as i32;

        if min_x > max_x || min_y > max_y {
            continue;
        }
        let edge0_a = v2.screen.y - v1.screen.y;
        let edge0_b = v1.screen.x - v2.screen.x;
        let edge0_c = v2.screen.x * v1.screen.y - v1.screen.x * v2.screen.y;
        let edge1_a = v0.screen.y - v2.screen.y;
        let edge1_b = v2.screen.x - v0.screen.x;
        let edge1_c = v0.screen.x * v2.screen.y - v2.screen.x * v0.screen.y;
        let edge2_a = v1.screen.y - v0.screen.y;
        let edge2_b = v0.screen.x - v1.screen.x;
        let edge2_c = v1.screen.x * v0.screen.y - v0.screen.x * v1.screen.y;
        let start_x = min_x as f32 + 0.5;
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let mut edge0 = edge0_a * start_x + edge0_b * py + edge0_c;
            let mut edge1 = edge1_a * start_x + edge1_b * py + edge1_c;
            let mut edge2 = edge2_a * start_x + edge2_b * py + edge2_c;
            for x in min_x..=max_x {
                let w0 = edge0 * inv_area;
                let w1 = edge1 * inv_area;
                let w2 = edge2 * inv_area;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }
                let depth = v0.depth * w0 + v1.depth * w1 + v2.depth * w2;
                if !(0.0..=1.0).contains(&depth) {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }
                let idx = (y as usize) * width_usize + (x as usize);
                if depth_less(subpixels.depth[idx], depth) {
                    subpixels.depth[idx] = depth;
                    let world_pos = v0.world_pos * w0 + v1.world_pos * w1 + v2.world_pos * w2;
                    let world_normal =
                        (v0.world_normal * w0 + v1.world_normal * w1 + v2.world_normal * w2)
                            .normalize_or_zero();
                    let uv0 = v0.uv0 * w0 + v1.uv0 * w1 + v2.uv0 * w2;
                    let vertex_color = [
                        v0.vertex_color[0] * w0 + v1.vertex_color[0] * w1 + v2.vertex_color[0] * w2,
                        v0.vertex_color[1] * w0 + v1.vertex_color[1] * w1 + v2.vertex_color[1] * w2,
                        v0.vertex_color[2] * w0 + v1.vertex_color[2] * w1 + v2.vertex_color[2] * w2,
                        v0.vertex_color[3] * w0 + v1.vertex_color[3] * w1 + v2.vertex_color[3] * w2,
                    ];
                    let material_index = v0
                        .material_index
                        .or(v1.material_index)
                        .or(v2.material_index);
                    let albedo =
                        sample_material_albedo(scene, material_index, uv0, vertex_color, config);
                    let lighting = shade_lighting(world_normal, world_pos, shading).clamp(0.0, 1.0);
                    let fog = depth.powf(1.7) * shading.fog_strength * contrast.fog_scale;
                    let base_light = (lighting * (1.0 - fog.clamp(0.0, 1.0))).clamp(0.0, 1.0);
                    let shaded_rgb = [
                        albedo[0] * base_light,
                        albedo[1] * base_light,
                        albedo[2] * base_light,
                    ];
                    let base = luminance(shaded_rgb);
                    push_histogram(histogram, histogram_count, base);
                    let intensity =
                        tone_map_intensity(base, threshold.floor, threshold.gamma, exposure);
                    subpixels.intensity[idx] = intensity;
                    subpixels.color_rgb[idx] = if config.material_color {
                        to_display_rgb(scale_rgb(shaded_rgb, intensity.max(0.15)))
                    } else {
                        model_color_for_intensity(intensity, palette)
                    };
                    stats.pixels_drawn += 1;
                }
                edge0 += edge0_a;
                edge1 += edge1_a;
                edge2 += edge2_a;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BrailleThresholds {
    on_threshold: f32,
    min_visible: f32,
    floor: f32,
    gamma: f32,
}

fn braille_thresholds(profile: BrailleProfile, safe_boost: bool) -> BrailleThresholds {
    match profile {
        BrailleProfile::Safe => {
            let mut value = BrailleThresholds {
                on_threshold: 0.10,
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
            on_threshold: 0.13,
            min_visible: 0.09,
            floor: 0.10,
            gamma: 0.90,
        },
        BrailleProfile::Dense => BrailleThresholds {
            on_threshold: 0.16,
            min_visible: 0.12,
            floor: 0.07,
            gamma: 0.98,
        },
    }
}

fn compose_braille_cells(
    frame: &mut FrameBuffers,
    subpixels: &BrailleSubpixelBuffers,
    config: &RenderConfig,
    palette: ThemePalette,
    threshold: BrailleThresholds,
) {
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

fn update_safe_visibility_state(scratch: &mut RenderScratch, profile: BrailleProfile, ratio: f32) {
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

fn push_histogram(histogram: &mut [u32; 64], count: &mut u32, value: f32) {
    let v = value.clamp(0.0, 1.0);
    let idx = ((v * ((histogram.len() - 1) as f32)).round() as usize).min(histogram.len() - 1);
    histogram[idx] = histogram[idx].saturating_add(1);
    *count = count.saturating_add(1);
}

fn percentile_from_histogram(histogram: &[u32; 64], count: u32, q: f32) -> f32 {
    if count == 0 {
        return 0.5;
    }
    let target = ((count as f32) * q.clamp(0.0, 1.0)).ceil() as u32;
    let mut acc = 0_u32;
    for (i, bin) in histogram.iter().enumerate() {
        acc = acc.saturating_add(*bin);
        if acc >= target {
            return (i as f32) / ((histogram.len() - 1) as f32);
        }
    }
    1.0
}

fn update_exposure_from_histogram(
    exposure: &mut f32,
    histogram: &[u32; 64],
    count: u32,
    profile: BrailleProfile,
) {
    if count == 0 {
        return;
    }
    let p85 = percentile_from_histogram(histogram, count, 0.85).max(1e-3);
    let desired_mid = match profile {
        BrailleProfile::Safe => 0.70,
        BrailleProfile::Normal => 0.72,
        BrailleProfile::Dense => 0.75,
    };
    let target = (desired_mid / p85).clamp(0.55, 2.6);
    *exposure = (*exposure + (target - *exposure) * 0.12).clamp(0.35, 3.2);
}

fn tone_map_intensity(raw: f32, floor: f32, gamma: f32, exposure: f32) -> f32 {
    let boosted = (raw.clamp(0.0, 1.0) * exposure).clamp(0.0, 1.4);
    let mapped = floor + (1.0 - floor) * boosted.clamp(0.0, 1.0).powf(gamma);
    mapped.clamp(0.0, 1.0)
}

fn exposure_bias_multiplier(bias: f32) -> f32 {
    let clamped = bias.clamp(-0.5, 0.8);
    (2.0_f32).powf(clamped).clamp(0.70, 1.80)
}

fn glyph_for_intensity(intensity: f32, charset: &[char]) -> char {
    if charset.is_empty() {
        return ' ';
    }
    let last = charset.len().saturating_sub(1);
    let index = ((intensity * (last as f32)).round() as usize).min(last);
    charset[index]
}

fn visible_cell_ratio(frame: &FrameBuffers) -> f32 {
    let total = frame.depth.len();
    if total == 0 {
        return 0.0;
    }
    let visible = frame.depth.iter().filter(|depth| depth.is_finite()).count();
    (visible as f32) / (total as f32)
}

fn apply_visible_metrics(stats: &mut RenderStats, frame: &FrameBuffers) {
    stats.visible_cell_ratio = visible_cell_ratio(frame);
    stats.visible_centroid_px = stats.root_screen_px;
    stats.visible_bbox_px = None;
    stats.visible_bbox_aspect = 0.0;
    stats.visible_height_ratio = 0.0;
    if frame.width == 0 || frame.height == 0 {
        return;
    }

    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if !frame.depth[idx].is_finite() {
                continue;
            }
            visible = visible.saturating_add(1);
            sum_x += x as f32 + 0.5;
            sum_y += y as f32 + 0.5;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if visible == 0 {
        return;
    }

    let silhouette_centroid = (sum_x / visible as f32, sum_y / visible as f32);
    if stats.visible_centroid_px.is_none() {
        stats.visible_centroid_px = Some(silhouette_centroid);
    }
    stats.visible_bbox_px = Some((
        min_x as u16,
        min_y as u16,
        max_x.min(width.saturating_sub(1)) as u16,
        max_y.min(height.saturating_sub(1)) as u16,
    ));
    let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
    let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
    stats.visible_bbox_aspect = if bbox_h > f32::EPSILON {
        bbox_w / bbox_h
    } else {
        0.0
    };
    stats.visible_height_ratio = (bbox_h / (frame.height as f32)).clamp(0.0, 1.0);
}

fn perp_dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn shade_lighting(normal: Vec3, world_pos: Vec3, shading: ShadingParams) -> f32 {
    let n = normal.normalize_or_zero();
    let l = shading.light_dir;
    let v = (shading.camera_pos - world_pos).normalize_or_zero();
    let h = (l + v).normalize_or_zero();

    let diffuse = n.dot(l).max(0.0) * shading.diffuse_strength;
    let specular = if shading.specular_strength > 0.0 {
        n.dot(h).max(0.0).powf(shading.specular_power) * shading.specular_strength
    } else {
        0.0
    };
    let rim = if shading.rim_strength > 0.0 {
        (1.0 - n.dot(v).max(0.0)).powf(shading.rim_power) * shading.rim_strength
    } else {
        0.0
    };

    let lit = shading.ambient + diffuse + specular + rim;
    lit.clamp(0.0, 1.0)
}

const BRAILLE_RAMP: &str = "⠀⠂⠆⠖⠶⠷⠿⡿⣿";
const ADAPTIVE_ASCII_LOW: [char; 9] = [' ', '.', ':', '=', '+', '*', '#', '%', '@'];
const ADAPTIVE_ASCII_NORMAL: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
const ADAPTIVE_ASCII_HIGH: [char; 11] = [' ', ' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

fn contrast_params(config: &RenderConfig, cells: usize) -> ContrastParams {
    match config.contrast_profile {
        ContrastProfile::Fixed => ContrastParams {
            floor: config.contrast_floor.clamp(0.0, 0.4),
            gamma: config.contrast_gamma.clamp(0.55, 1.40),
            fog_scale: config.fog_scale.clamp(0.25, 1.5),
        },
        ContrastProfile::Adaptive => {
            let (bucket_floor, bucket_gamma, bucket_fog) = if cells < 6_000 {
                (0.18, 0.78, 0.55)
            } else if cells < 12_000 {
                (0.12, 0.86, 0.75)
            } else {
                (0.08, 0.92, 1.00)
            };
            let floor_scale = (config.contrast_floor / 0.10).clamp(0.5, 2.0);
            let gamma_scale = (config.contrast_gamma / 0.90).clamp(0.6, 1.5);
            ContrastParams {
                floor: (bucket_floor * floor_scale).clamp(0.02, 0.35),
                gamma: (bucket_gamma * gamma_scale).clamp(0.55, 1.20),
                fog_scale: (bucket_fog * config.fog_scale.clamp(0.25, 1.5)).clamp(0.25, 1.5),
            }
        }
    }
}

fn select_charset<'a>(config: &RenderConfig, fallback: &'a [char], cells: usize) -> &'a [char] {
    if config.mode != RenderMode::Ascii {
        return fallback;
    }
    if config.charset != DEFAULT_CHARSET {
        return fallback;
    }
    if config.contrast_profile == ContrastProfile::Fixed {
        return fallback;
    }
    if cells < 6_000 {
        &ADAPTIVE_ASCII_HIGH
    } else if cells < 12_000 {
        &ADAPTIVE_ASCII_NORMAL
    } else {
        &ADAPTIVE_ASCII_LOW
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BrailleProfile, CellAspectMode, ColorMode, RenderConfig, ThemeStyle};

    #[test]
    fn adaptive_contrast_is_in_range() {
        let config = RenderConfig::default();
        for cells in [4_000, 8_000, 20_000] {
            let params = contrast_params(&config, cells);
            assert!((0.0..=0.4).contains(&params.floor));
            assert!((0.5..=1.3).contains(&params.gamma));
            assert!((0.2..=1.5).contains(&params.fog_scale));
        }
    }

    #[test]
    fn glyph_for_intensity_is_monotonic() {
        let charset = &ADAPTIVE_ASCII_NORMAL;
        let low = glyph_for_intensity(0.2, charset);
        let high = glyph_for_intensity(0.8, charset);
        let idx_low = charset.iter().position(|c| *c == low).unwrap_or_default();
        let idx_high = charset.iter().position(|c| *c == high).unwrap_or_default();
        assert!(idx_high >= idx_low);
    }

    #[test]
    fn adaptive_charset_uses_default_only() {
        let mut config = RenderConfig {
            cell_aspect_mode: CellAspectMode::Auto,
            ..RenderConfig::default()
        };
        let fallback = vec!['.', '#'];
        let picked = select_charset(&config, &fallback, 5_000);
        assert_ne!(picked, fallback.as_slice());

        config.charset = " .:-=+*#%@X".to_owned();
        let picked_custom = select_charset(&config, &fallback, 5_000);
        assert_eq!(picked_custom, fallback.as_slice());
    }

    #[test]
    fn braille_safe_guard_promotes_non_empty_cell() {
        let mut frame = FrameBuffers::new(1, 1);
        let mut sub = BrailleSubpixelBuffers::default();
        sub.resize(2, 4);
        sub.clear();
        sub.intensity[0] = 0.07;
        sub.color_rgb[0] = [220, 180, 120];
        sub.depth[0] = 0.42;
        let config = RenderConfig {
            mode: RenderMode::Braille,
            color_mode: ColorMode::Ansi,
            braille_profile: BrailleProfile::Safe,
            theme_style: ThemeStyle::Theater,
            ..RenderConfig::default()
        };
        compose_braille_cells(
            &mut frame,
            &sub,
            &config,
            theme_palette(config.theme_style),
            braille_thresholds(config.braille_profile, false),
        );
        assert_ne!(frame.glyphs[0], ' ');
        assert_ne!(frame.glyphs[0], '⠀');
    }

    #[test]
    fn ansi_encoder_emits_escape_sequences() {
        let mut frame = FrameBuffers::new(2, 1);
        frame.has_color = true;
        frame.glyphs[0] = '@';
        frame.glyphs[1] = '#';
        frame.fg_rgb[0] = [255, 0, 0];
        frame.fg_rgb[1] = [0, 255, 0];
        let mut out = String::new();
        encode_ansi_frame(&frame, &mut out);
        assert!(out.contains("\x1b[38;2;255;0;0m"));
        assert!(out.contains("\x1b[38;2;0;255;0m"));
        assert!(out.contains("@"));
        assert!(out.contains("#"));
    }

    #[test]
    fn exposure_bias_multiplier_is_monotonic() {
        let low = exposure_bias_multiplier(-0.4);
        let mid = exposure_bias_multiplier(0.0);
        let high = exposure_bias_multiplier(0.6);
        assert!(low < mid);
        assert!(mid < high);
    }

    #[test]
    fn visible_ratio_uses_depth_buffer() {
        let mut frame = FrameBuffers::new(4, 1);
        frame.depth = vec![f32::INFINITY, 0.3, f32::INFINITY, 0.7];
        let ratio = visible_cell_ratio(&frame);
        assert!((ratio - 0.5).abs() < 1e-6);
    }
}
