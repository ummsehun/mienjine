use glam::{Mat4, Quat, Vec3};

use crate::animation::AnimationClip;

pub const DEFAULT_CHARSET: &str = " .:-=+*#%@";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Ascii,
    Braille,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellAspectMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastProfile {
    Adaptive,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSpeedMode {
    AutoDurationFit,
    Realtime1x,
}

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
    pub mode: RenderMode,
    pub charset: String,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub fps_cap: u32,
    pub ambient: f32,
    pub diffuse_strength: f32,
    pub specular_strength: f32,
    pub specular_power: f32,
    pub rim_strength: f32,
    pub rim_power: f32,
    pub fog_strength: f32,
    pub contrast_profile: ContrastProfile,
    pub contrast_floor: f32,
    pub contrast_gamma: f32,
    pub fog_scale: f32,
    pub triangle_stride: usize,
    pub min_triangle_area_px2: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
            mode: RenderMode::Ascii,
            charset: DEFAULT_CHARSET.to_owned(),
            cell_aspect: 0.5,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            fps_cap: 30,
            ambient: 0.12,
            diffuse_strength: 0.95,
            specular_strength: 0.25,
            specular_power: 24.0,
            rim_strength: 0.22,
            rim_power: 2.0,
            fog_strength: 0.20,
            contrast_profile: ContrastProfile::Adaptive,
            contrast_floor: 0.10,
            contrast_gamma: 0.90,
            fog_scale: 1.0,
            triangle_stride: 1,
            min_triangle_area_px2: 0.0,
        }
    }
}

pub fn estimate_cell_aspect_from_window(
    columns: u16,
    rows: u16,
    width_px: u16,
    height_px: u16,
) -> Option<f32> {
    if columns == 0 || rows == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let cell_w = (width_px as f32) / (columns as f32);
    let cell_h = (height_px as f32) / (rows as f32);
    if cell_h <= f32::EPSILON {
        return None;
    }
    Some(cell_w / cell_h)
}

pub fn resolve_cell_aspect(config: &RenderConfig, detected: Option<f32>) -> f32 {
    let trim = config.cell_aspect_trim.clamp(0.70, 1.30);
    match config.cell_aspect_mode {
        CellAspectMode::Manual => config.cell_aspect.clamp(0.30, 1.20),
        CellAspectMode::Auto => detected
            .map(|value| (value * trim).clamp(0.30, 1.20))
            .unwrap_or_else(|| config.cell_aspect.clamp(0.30, 1.20)),
    }
}

#[derive(Debug, Clone)]
pub struct MeshCpu {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
    pub joints4: Option<Vec<[u16; 4]>>,
    pub weights4: Option<Vec<[f32; 4]>>,
}

impl MeshCpu {
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }
}

#[derive(Debug, Clone)]
pub struct SkinCpu {
    pub joints: Vec<usize>,
    pub inverse_bind_mats: Vec<Mat4>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Option<String>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub base_translation: Vec3,
    pub base_rotation: Quat,
    pub base_scale: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct NodePose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl NodePose {
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl From<&Node> for NodePose {
    fn from(value: &Node) -> Self {
        Self {
            translation: value.base_translation,
            rotation: value.base_rotation,
            scale: value.base_scale,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshInstance {
    pub mesh_index: usize,
    pub node_index: usize,
    pub skin_index: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct SceneCpu {
    pub meshes: Vec<MeshCpu>,
    pub skins: Vec<SkinCpu>,
    pub nodes: Vec<Node>,
    pub mesh_instances: Vec<MeshInstance>,
    pub animations: Vec<AnimationClip>,
}

impl SceneCpu {
    pub fn total_vertices(&self) -> usize {
        self.meshes.iter().map(MeshCpu::vertex_count).sum()
    }

    pub fn total_triangles(&self) -> usize {
        self.meshes.iter().map(MeshCpu::triangle_count).sum()
    }

    pub fn total_joints(&self) -> usize {
        self.skins.iter().map(|s| s.joints.len()).sum()
    }

    pub fn animation_index_by_selector(&self, selector: Option<&str>) -> Option<usize> {
        let selector = selector?;
        if let Ok(index) = selector.parse::<usize>() {
            return (index < self.animations.len()).then_some(index);
        }
        self.animations
            .iter()
            .position(|clip| clip.name.as_deref() == Some(selector))
    }
}

pub fn cube_scene() -> SceneCpu {
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(12);

    let faces = [
        (
            Vec3::new(0.0, 0.0, 1.0),
            [
                Vec3::new(-1.0, -1.0, 1.0),
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(-1.0, 1.0, 1.0),
            ],
        ),
        (
            Vec3::new(0.0, 0.0, -1.0),
            [
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
            ],
        ),
        (
            Vec3::new(-1.0, 0.0, 0.0),
            [
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(-1.0, -1.0, 1.0),
                Vec3::new(-1.0, 1.0, 1.0),
                Vec3::new(-1.0, 1.0, -1.0),
            ],
        ),
        (
            Vec3::new(1.0, 0.0, 0.0),
            [
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0),
            ],
        ),
        (
            Vec3::new(0.0, 1.0, 0.0),
            [
                Vec3::new(-1.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(-1.0, 1.0, -1.0),
            ],
        ),
        (
            Vec3::new(0.0, -1.0, 0.0),
            [
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(-1.0, -1.0, 1.0),
            ],
        ),
    ];

    for (normal, verts) in faces {
        let base = positions.len() as u32;
        positions.extend(verts);
        normals.extend([normal; 4]);
        indices.push([base, base + 1, base + 2]);
        indices.push([base, base + 2, base + 3]);
    }
    let mesh = MeshCpu {
        positions,
        normals,
        indices,
        joints4: None,
        weights4: None,
    };

    let node = Node {
        name: Some("CubeRoot".to_owned()),
        parent: None,
        children: Vec::new(),
        base_translation: Vec3::ZERO,
        base_rotation: Quat::IDENTITY,
        base_scale: Vec3::ONE,
    };

    SceneCpu {
        meshes: vec![mesh],
        skins: Vec::new(),
        nodes: vec![node],
        mesh_instances: vec![MeshInstance {
            mesh_index: 0,
            node_index: 0,
            skin_index: None,
        }],
        animations: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_cell_aspect_formula_matches_expected_ratio() {
        let aspect = estimate_cell_aspect_from_window(120, 40, 1920, 1200)
            .expect("aspect should be available");
        let expected = (1920.0 / 120.0) / (1200.0 / 40.0);
        assert!((aspect - expected).abs() < 1e-6);
    }

    #[test]
    fn resolve_cell_aspect_auto_uses_detected_value_and_trim() {
        let mut cfg = RenderConfig {
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.10,
            ..RenderConfig::default()
        };
        let value = resolve_cell_aspect(&cfg, Some(0.82));
        assert!((value - 0.902).abs() < 1e-3);

        cfg.cell_aspect_trim = 2.0;
        let clamped = resolve_cell_aspect(&cfg, Some(1.0));
        assert!((clamped - 1.2).abs() < 1e-6);
    }

    #[test]
    fn resolve_cell_aspect_manual_ignores_detected() {
        let cfg = RenderConfig {
            cell_aspect_mode: CellAspectMode::Manual,
            cell_aspect: 0.58,
            ..RenderConfig::default()
        };
        let value = resolve_cell_aspect(&cfg, Some(0.91));
        assert!((value - 0.58).abs() < 1e-6);
    }
}
