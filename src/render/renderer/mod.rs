//! Terminal renderer for ASCII and Braille output.
//!
//! This module provides the main rendering pipeline for converting 3D scenes
//! into terminal-compatible ASCII or Braille representations.

use std::fmt::Write as _;

pub use super::common::exposure::exposure_bias_multiplier;
pub use super::common::glyph::GlyphRamp;

mod braille;
mod projection;
mod rasterization;
mod rasterization_braille;
mod rendering;
mod shading;
#[cfg(test)]
mod tests;

pub use rendering::render_frame;

use super::common::glyph::glyph_coverage;
use crate::scene::{AnsiQuantization, KittyPipelineMode, MaterialAlphaMode};

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub shadow: [u8; 3],
    pub mid: [u8; 3],
    pub highlight: [u8; 3],
    pub bg: [u8; 3],
}

#[derive(Debug, Default, Clone)]
pub struct BrailleSubpixelBuffers {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) depth: Vec<f32>,
    pub(super) intensity: Vec<f32>,
    pub(super) color_rgb: Vec<[u8; 3]>,
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: glam::Vec3,
    pub target: glam::Vec3,
    pub up: glam::Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: glam::Vec3::new(0.0, 1.2, 4.0),
            target: glam::Vec3::new(0.0, 1.0, 0.0),
            up: glam::Vec3::Y,
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

#[derive(Debug, Clone)]
pub struct PixelFrame {
    pub width_px: u32,
    pub height_px: u32,
    pub rgba8: Vec<u8>,
    pub depth: Vec<f32>,
    pub subject_mask: Vec<u8>,
}

impl PixelFrame {
    pub fn new(width_px: u32, height_px: u32) -> Self {
        let size = (width_px as usize).saturating_mul(height_px as usize);
        Self {
            width_px,
            height_px,
            rgba8: vec![0; size.saturating_mul(4)],
            depth: vec![f32::INFINITY; size],
            subject_mask: vec![0; size],
        }
    }
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

    pub fn write_ansi_text(&self, out: &mut String, quantization: AnsiQuantization) {
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
                let rgb = quantize_rgb(self.fg_rgb[idx], quantization);
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

fn quantize_rgb_q216(rgb: [u8; 3]) -> [u8; 3] {
    fn q(c: u8) -> u8 {
        let bucket = ((c as u16 * 5 + 127) / 255) as u8;
        bucket * 51
    }
    [q(rgb[0]), q(rgb[1]), q(rgb[2])]
}

fn quantize_rgb(rgb: [u8; 3], quantization: AnsiQuantization) -> [u8; 3] {
    match quantization {
        AnsiQuantization::Q216 => quantize_rgb_q216(rgb),
        AnsiQuantization::Off => rgb,
    }
}

pub fn pixel_frame_from_cells(
    frame: &FrameBuffers,
    cell_px_w: u32,
    cell_px_h: u32,
    mode: KittyPipelineMode,
    background_rgb: [u8; 3],
) -> PixelFrame {
    if frame.glyphs.is_empty() {
        return PixelFrame::new(1, 1);
    }
    let cell_px_w = cell_px_w.max(1);
    let cell_px_h = cell_px_h.max(1);
    let width_px = u32::from(frame.width).max(1).saturating_mul(cell_px_w);
    let height_px = u32::from(frame.height).max(1).saturating_mul(cell_px_h);
    let mut out = PixelFrame::new(width_px, height_px);
    let cols = usize::from(frame.width.max(1));
    for cy in 0..u32::from(frame.height.max(1)) {
        for cx in 0..u32::from(frame.width.max(1)) {
            let cell_idx = (cy as usize)
                .saturating_mul(cols)
                .saturating_add(cx as usize)
                .min(frame.glyphs.len().saturating_sub(1));
            let glyph = frame.glyphs[cell_idx];
            let rgb = frame
                .fg_rgb
                .get(cell_idx)
                .copied()
                .unwrap_or([255, 255, 255]);
            let depth = frame.depth.get(cell_idx).copied().unwrap_or(f32::INFINITY);
            let visible = depth.is_finite();
            let coverage = match mode {
                KittyPipelineMode::RealPixel => {
                    if visible {
                        1.0
                    } else {
                        0.0
                    }
                }
                KittyPipelineMode::GlyphCompat => glyph_coverage(glyph),
            };
            let lit = (coverage.clamp(0.0, 1.0) * (cell_px_w * cell_px_h) as f32).round() as u32;
            let mut wrote = 0_u32;
            for py in 0..cell_px_h {
                for px in 0..cell_px_w {
                    let x = cx * cell_px_w + px;
                    let y = cy * cell_px_h + py;
                    let idx = (y as usize)
                        .saturating_mul(width_px as usize)
                        .saturating_add(x as usize);
                    let rgba_idx = idx.saturating_mul(4);
                    let is_fg = wrote < lit;
                    let color = if is_fg { rgb } else { background_rgb };
                    out.rgba8[rgba_idx] = color[0];
                    out.rgba8[rgba_idx + 1] = color[1];
                    out.rgba8[rgba_idx + 2] = color[2];
                    out.rgba8[rgba_idx + 3] = 255;
                    out.depth[idx] = depth;
                    out.subject_mask[idx] = if visible && is_fg { 255 } else { 0 };
                    wrote += 1;
                }
            }
        }
    }
    out
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
    pub subject_visible_ratio: f32,
    pub subject_visible_height_ratio: f32,
    pub subject_centroid_px: Option<(f32, f32)>,
    pub subject_bbox_px: Option<(u16, u16, u16, u16)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RasterPass {
    Opaque,
    Mask,
    Blend,
}

impl RasterPass {
    fn all() -> [Self; 3] {
        [Self::Opaque, Self::Mask, Self::Blend]
    }

    fn matches(self, mode: MaterialAlphaMode) -> bool {
        matches!(
            (self, mode),
            (Self::Opaque, MaterialAlphaMode::Opaque)
                | (Self::Mask, MaterialAlphaMode::Mask)
                | (Self::Blend, MaterialAlphaMode::Blend)
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProjectedVertex {
    pub screen: glam::Vec2,
    pub depth: f32,
    pub world_pos: glam::Vec3,
    pub world_normal: glam::Vec3,
    pub uv0: glam::Vec2,
    pub uv1: glam::Vec2,
    pub vertex_color: [f32; 4],
    pub material_index: Option<usize>,
}

#[derive(Debug, Default)]
pub struct RenderScratch {
    pub(super) projected_vertices: Vec<Option<ProjectedVertex>>,
    pub(super) triangle_order: Vec<usize>,
    pub(super) triangle_depth_sorted: Vec<(usize, f32)>,
    pub(super) subject_depth_cells: Vec<f32>,
    pub(super) braille_subpixels: BrailleSubpixelBuffers,
    pub(super) exposure: f32,
    pub(super) safe_low_visibility_streak: u32,
    pub(super) safe_high_visibility_streak: u32,
    pub(super) safe_boost_active: bool,
}

impl RenderScratch {
    pub fn with_capacity(vertex_capacity: usize) -> Self {
        Self {
            projected_vertices: Vec::with_capacity(vertex_capacity),
            triangle_order: Vec::new(),
            triangle_depth_sorted: Vec::new(),
            subject_depth_cells: Vec::new(),
            braille_subpixels: BrailleSubpixelBuffers::default(),
            exposure: 1.0,
            safe_low_visibility_streak: 0,
            safe_high_visibility_streak: 0,
            safe_boost_active: false,
        }
    }

    pub(super) fn prepare_projected_vertices(
        &mut self,
        vertex_count: usize,
    ) -> &mut [Option<ProjectedVertex>] {
        self.projected_vertices.clear();
        self.projected_vertices.resize(vertex_count, None);
        self.projected_vertices.as_mut_slice()
    }

    pub fn reset_exposure(&mut self) {
        self.exposure = 1.0;
        self.safe_low_visibility_streak = 0;
        self.safe_high_visibility_streak = 0;
        self.safe_boost_active = false;
    }
}

impl BrailleSubpixelBuffers {
    pub(super) fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width).saturating_mul(usize::from(height));
        self.depth.resize(size, f32::INFINITY);
        self.intensity.resize(size, 0.0);
        self.color_rgb.resize(size, [0, 0, 0]);
    }

    pub(super) fn clear(&mut self) {
        self.depth.fill(f32::INFINITY);
        self.intensity.fill(0.0);
        self.color_rgb.fill([0, 0, 0]);
    }
}

pub fn encode_ansi_frame(frame: &FrameBuffers, out: &mut String, quantization: AnsiQuantization) {
    frame.write_ansi_text(out, quantization);
}
