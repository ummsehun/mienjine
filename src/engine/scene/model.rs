use glam::{Mat4, Quat, Vec2, Vec3};

use crate::animation::AnimationClip;

use super::{
    MaterialAlphaMode, TextureColorSpace, TextureFilterMode, TextureWrapMode, UvTransform2D,
};

#[derive(Debug, Clone)]
pub struct MorphTargetCpu {
    pub name: Option<String>,
    pub position_deltas: Vec<Vec3>,
    pub normal_deltas: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct MeshCpu {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uv0: Option<Vec<Vec2>>,
    pub uv1: Option<Vec<Vec2>>,
    pub colors_rgba: Option<Vec<[f32; 4]>>,
    pub material_index: Option<usize>,
    pub indices: Vec<[u32; 3]>,
    pub joints4: Option<Vec<[u16; 4]>>,
    pub weights4: Option<Vec<[f32; 4]>>,
    pub sdef_vertices: Option<Vec<Option<SdefVertexCpu>>>,
    pub morph_targets: Vec<MorphTargetCpu>,
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

#[derive(Debug, Clone, Copy)]
pub struct SdefVertexCpu {
    pub bone_index_1: u16,
    pub bone_index_2: u16,
    pub bone_weight_1: f32,
    pub c: Vec3,
    pub r0: Vec3,
    pub r1: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialMorphFormula {
    Multiply = 0,
    Add = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialToonSource {
    Separate(usize),
    BuiltIn(u8),
}

#[derive(Debug, Clone)]
pub struct MaterialMorphOp {
    pub target_material_index: i32,
    pub formula: MaterialMorphFormula,
    pub diffuse: [f32; 4],
    pub specular: [f32; 3],
    pub specular_factor: f32,
    pub ambient: [f32; 3],
    pub edge_color: [f32; 4],
    pub edge_size: f32,
    pub texture_factor: [f32; 4],
    pub sphere_texture_factor: [f32; 4],
    pub toon_texture_factor: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct MaterialMorphCpu {
    pub name: String,
    pub operations: Vec<MaterialMorphOp>,
}

#[derive(Debug, Clone)]
pub struct MaterialCpu {
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<usize>,
    pub base_color_tex_coord: u32,
    pub base_color_uv_transform: Option<UvTransform2D>,
    pub base_color_wrap_s: TextureWrapMode,
    pub base_color_wrap_t: TextureWrapMode,
    pub base_color_min_filter: TextureFilterMode,
    pub base_color_mag_filter: TextureFilterMode,
    pub sphere_texture: Option<usize>,
    pub toon_source: Option<MaterialToonSource>,
    pub emissive_factor: [f32; 3],
    pub alpha_mode: MaterialAlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

#[derive(Debug, Clone)]
pub struct TextureLevelCpu {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TextureCpu {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    pub source_format: String,
    pub color_space: TextureColorSpace,
    pub mip_levels: Vec<TextureLevelCpu>,
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

#[derive(Debug, Clone)]
pub struct MeshInstance {
    pub mesh_index: usize,
    pub node_index: usize,
    pub skin_index: Option<usize>,
    pub default_morph_weights: Vec<f32>,
    pub layer: MeshLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshLayer {
    Subject,
    Stage,
}

#[derive(Debug, Clone, Default)]
pub struct SceneCpu {
    pub meshes: Vec<MeshCpu>,
    pub materials: Vec<MaterialCpu>,
    pub textures: Vec<TextureCpu>,
    pub skins: Vec<SkinCpu>,
    pub nodes: Vec<Node>,
    pub mesh_instances: Vec<MeshInstance>,
    pub animations: Vec<AnimationClip>,
    pub root_center_node: Option<usize>,
    pub pmx_rig_meta: Option<crate::engine::pmx_rig::PmxRigMeta>,
    pub pmx_physics_meta: Option<crate::engine::pmx_rig::PmxPhysicsMeta>,
    pub material_morphs: Vec<MaterialMorphCpu>,
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
        uv0: None,
        uv1: None,
        colors_rgba: None,
        material_index: None,
        indices,
        joints4: None,
        weights4: None,
        sdef_vertices: None,
        morph_targets: Vec::new(),
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
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![node],
        mesh_instances: vec![MeshInstance {
            mesh_index: 0,
            node_index: 0,
            skin_index: None,
            default_morph_weights: Vec::new(),
            layer: MeshLayer::Subject,
        }],
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    }
}
