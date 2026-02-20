use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};

use crate::math::{depth_less, perspective_matrix};
use crate::scene::{ContrastProfile, DEFAULT_CHARSET, MeshCpu, RenderConfig, RenderMode, SceneCpu};

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
}

impl FrameBuffers {
    pub fn new(width: u16, height: u16) -> Self {
        let size = usize::from(width).saturating_mul(usize::from(height));
        Self {
            width,
            height,
            glyphs: vec![' '; size],
            depth: vec![f32::INFINITY; size],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width).saturating_mul(usize::from(height));
        self.glyphs.resize(size, ' ');
        self.depth.resize(size, f32::INFINITY);
    }

    pub fn clear(&mut self, glyph: char) {
        self.glyphs.fill(glyph);
        self.depth.fill(f32::INFINITY);
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
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RenderStats {
    pub triangles_total: usize,
    pub triangles_culled: usize,
    pub pixels_drawn: usize,
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
}

impl RenderScratch {
    pub fn with_capacity(vertex_capacity: usize) -> Self {
        Self {
            projected_vertices: Vec::with_capacity(vertex_capacity),
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

#[derive(Debug, Clone, Copy)]
struct ProjectedVertex {
    screen: Vec2,
    depth: f32,
    intensity: f32,
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
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    frame.clear(' ');
    if frame.width == 0 || frame.height == 0 {
        return RenderStats::default();
    }

    let cells = usize::from(frame.width).saturating_mul(usize::from(frame.height));
    let contrast = contrast_params(config, cells);
    let charset = select_charset(config, glyph_ramp.chars(), cells);
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
        fog_strength: config.fog_strength.clamp(0.0, 1.0),
    };

    let mut stats = RenderStats::default();

    for instance in &scene.mesh_instances {
        let mesh = match scene.meshes.get(instance.mesh_index) {
            Some(mesh) => mesh,
            None => continue,
        };
        let node_global = global_matrices
            .get(instance.node_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        let model = model_rotation * node_global;
        let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
        let projected_vertices = scratch.prepare_projected_vertices(mesh.positions.len());
        project_mesh_vertices(
            mesh,
            model,
            normal_matrix,
            view_projection,
            frame.width,
            frame.height,
            instance.skin_index.and_then(|i| skin_matrices.get(i)),
            shading,
            projected_vertices,
        );
        stats.triangles_total += mesh.indices.len();
        rasterize_mesh(
            mesh,
            projected_vertices,
            frame,
            charset,
            config,
            &mut stats,
            shading,
            contrast,
        );
    }

    stats
}

fn project_mesh_vertices(
    mesh: &MeshCpu,
    model: Mat4,
    normal_matrix: Mat3,
    view_projection: Mat4,
    width: u16,
    height: u16,
    skin_matrices: Option<&Vec<Mat4>>,
    shading: ShadingParams,
    projected_vertices: &mut [Option<ProjectedVertex>],
) {
    for (index, position) in mesh.positions.iter().enumerate() {
        let normal = mesh
            .normals
            .get(index)
            .copied()
            .unwrap_or(Vec3::new(0.0, 1.0, 0.0));
        let (skinned_pos, skinned_normal) =
            apply_skin(mesh, index, *position, normal, skin_matrices);
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
        let intensity = shade_lighting(world_normal, world_pos, shading);
        projected_vertices[index] = Some(ProjectedVertex {
            screen,
            depth,
            intensity,
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

fn rasterize_mesh(
    mesh: &MeshCpu,
    projected_vertices: &[Option<ProjectedVertex>],
    frame: &mut FrameBuffers,
    charset: &[char],
    config: &RenderConfig,
    stats: &mut RenderStats,
    shading: ShadingParams,
    contrast: ContrastParams,
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
                    let lighting =
                        (v0.intensity * w0 + v1.intensity * w1 + v2.intensity * w2).clamp(0.0, 1.0);
                    let fog = depth.powf(1.7) * shading.fog_strength * contrast.fog_scale;
                    let base = (lighting * (1.0 - fog.clamp(0.0, 1.0))).clamp(0.0, 1.0);
                    let intensity = (contrast.floor
                        + (1.0 - contrast.floor) * base.powf(contrast.gamma))
                    .clamp(0.0, 1.0);
                    frame.glyphs[idx] = glyph_for_intensity(intensity, charset);
                    stats.pixels_drawn += 1;
                }
                edge0 += edge0_a;
                edge1 += edge1_a;
                edge2 += edge2_a;
            }
        }
    }
}

fn glyph_for_intensity(intensity: f32, charset: &[char]) -> char {
    if charset.is_empty() {
        return ' ';
    }
    let last = charset.len().saturating_sub(1);
    let index = ((intensity * (last as f32)).round() as usize).min(last);
    charset[index]
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
    use crate::scene::{CellAspectMode, RenderConfig};

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
}
