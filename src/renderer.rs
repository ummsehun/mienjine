use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};

use crate::math::{barycentric, depth_less, perspective_matrix};
use crate::scene::{MeshCpu, RenderConfig, RenderMode, SceneCpu};

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
            glyph_ramp.chars(),
            &mut stats,
            shading,
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
    stats: &mut RenderStats,
    shading: ShadingParams,
) {
    let width = i32::from(frame.width);
    let height = i32::from(frame.height);
    if width <= 0 || height <= 0 {
        return;
    }

    for tri in &mesh.indices {
        let (Some(v0), Some(v1), Some(v2)) = (
            projected_vertices.get(tri[0] as usize).copied().flatten(),
            projected_vertices.get(tri[1] as usize).copied().flatten(),
            projected_vertices.get(tri[2] as usize).copied().flatten(),
        ) else {
            continue;
        };

        let signed_area = perp_dot(v1.screen - v0.screen, v2.screen - v0.screen);
        if signed_area >= -1e-6 {
            stats.triangles_culled += 1;
            continue;
        }

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

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let Some([w0, w1, w2]) = barycentric(point, v0.screen, v1.screen, v2.screen) else {
                    continue;
                };
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }

                let depth = v0.depth * w0 + v1.depth * w1 + v2.depth * w2;
                if !(0.0..=1.0).contains(&depth) {
                    continue;
                }
                let idx = (y as usize) * (width as usize) + (x as usize);
                if depth_less(frame.depth[idx], depth) {
                    frame.depth[idx] = depth;
                    let lighting =
                        (v0.intensity * w0 + v1.intensity * w1 + v2.intensity * w2).clamp(0.0, 1.0);
                    let fog = depth.powf(1.7) * shading.fog_strength;
                    let intensity = (lighting * (1.0 - fog)).clamp(0.0, 1.0);
                    frame.glyphs[idx] = glyph_for_intensity(intensity, charset);
                    stats.pixels_drawn += 1;
                }
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
    let contrasted = lit.powf(0.9);
    contrasted.clamp(0.0, 1.0)
}

const BRAILLE_RAMP: &str = "⠀⠂⠆⠖⠶⠷⠿⡿⣿";
