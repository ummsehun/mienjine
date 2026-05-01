mod mesh_morph;
mod rig_meta;

use std::{collections::HashSet, path::Path};

use anyhow::{Result, bail};
use glam::{Quat, Vec2, Vec3};

use crate::runtime::state::derive_pmx_profile;
use crate::scene::{
    MaterialAlphaMode, MaterialCpu, MaterialToonSource, MeshInstance, MeshLayer, Node, SceneCpu,
    SkinCpu, TextureFilterMode, TextureWrapMode,
};
use crate::shared::pmx_log;

use super::pmx_support::{
    compute_pmx_inverse_bind_mats, convert_vertex_weight, extract_material_morphs,
    extract_pmx_physics_metadata, extract_pmx_rig_metadata,
};
use super::texture_utils::load_pmx_texture;
use super::util::find_root_center_node;
use mesh_morph::build_mesh_for_material;
use rig_meta::{PmxMorphStats, log_pmx_parity_report};

pub(super) use mesh_morph::resolve_pmx_texture_path;

pub(super) fn load_pmx_impl(path: &Path) -> Result<SceneCpu> {
    let model_info_loader = PMXUtil::pmx_loader::PMXLoader::open(path);
    let (_model_info, vertices_loader): (
        PMXUtil::pmx_types::PMXModelInfo,
        PMXUtil::pmx_loader::VerticesLoader,
    ) = model_info_loader.read_pmx_model_info();

    let (vertices, faces_loader): (
        Vec<PMXUtil::pmx_types::PMXVertex>,
        PMXUtil::pmx_loader::FacesLoader,
    ) = vertices_loader.read_pmx_vertices();

    let (faces, textures_loader): (
        Vec<PMXUtil::pmx_types::PMXFace>,
        PMXUtil::pmx_loader::TexturesLoader,
    ) = faces_loader.read_pmx_faces();

    let (texture_list, materials_loader): (
        PMXUtil::pmx_types::PMXTextureList,
        PMXUtil::pmx_loader::MaterialsLoader,
    ) = textures_loader.read_texture_list();

    let (materials, bones_loader): (
        Vec<PMXUtil::pmx_types::PMXMaterial>,
        PMXUtil::pmx_loader::BonesLoader,
    ) = materials_loader.read_pmx_materials();

    let (bones, morphs_loader): (
        Vec<PMXUtil::pmx_types::PMXBone>,
        PMXUtil::pmx_loader::MorphsLoader,
    ) = bones_loader.read_pmx_bones();

    let (morphs, frames_loader): (
        Vec<PMXUtil::pmx_types::PMXMorph>,
        PMXUtil::pmx_loader::FrameLoader,
    ) = morphs_loader.read_pmx_morphs();

    let (_frames, rigid_loader): (
        Vec<PMXUtil::pmx_types::PMXFrame>,
        PMXUtil::pmx_loader::RigidLoader,
    ) = frames_loader.read_frames();

    let (rigid_bodies, joint_loader): (
        Vec<PMXUtil::pmx_types::PMXRigid>,
        PMXUtil::pmx_loader::JointLoader,
    ) = rigid_loader.read_rigids();

    let (joints, _soft_body_loader): (
        Vec<PMXUtil::pmx_types::PMXJoint>,
        Option<PMXUtil::pmx_loader::SoftBodyLoader>,
    ) = joint_loader.read_joints();

    if !rigid_bodies.is_empty() {
        pmx_log::warn(format!(
            "warning: PMX model has {} rigid bodies (physics approximated; joints/collision handling is limited).",
            rigid_bodies.len()
        ));
    }

    let mut scene = SceneCpu::default();

    let mut bone_nodes = Vec::with_capacity(bones.len());
    for (bone_index, bone) in bones.iter().enumerate() {
        let bone_position = Vec3::new(bone.position[0], bone.position[1], bone.position[2]);
        let translation = if bone.parent >= 0 {
            let parent_index = bone.parent as usize;
            if parent_index < bones.len() && parent_index != bone_index {
                let parent = &bones[parent_index];
                let parent_position =
                    Vec3::new(parent.position[0], parent.position[1], parent.position[2]);
                bone_position - parent_position
            } else {
                bone_position
            }
        } else {
            bone_position
        };
        bone_nodes.push(Node {
            name: Some(bone.name.clone()),
            name_en: Some(bone.english_name.clone()),
            parent: None,
            children: Vec::new(),
            base_translation: translation,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        });
    }
    for (i, bone) in bones.iter().enumerate() {
        if bone.parent >= 0 {
            let parent_idx = bone.parent as usize;
            if parent_idx < bone_nodes.len() && parent_idx != i {
                bone_nodes[parent_idx].children.push(i);
                bone_nodes[i].parent = Some(parent_idx);
            }
        }
    }
    scene.nodes = bone_nodes;

    let pmx_rig_meta = extract_pmx_rig_metadata(&bones);
    let pmx_physics_meta = extract_pmx_physics_metadata(&rigid_bodies, &joints);

    if !scene.nodes.is_empty() {
        let inverse_bind_mats = compute_pmx_inverse_bind_mats(&scene.nodes);
        scene.skins.push(SkinCpu {
            joints: (0..scene.nodes.len()).collect(),
            inverse_bind_mats,
        });
    }

    let texture_count = texture_list.textures.len();
    for (i, tex_path) in texture_list.textures.iter().enumerate() {
        let tex_name = Path::new(tex_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let full_path = resolve_pmx_texture_path(path, tex_path);
        let texture = load_pmx_texture(&full_path, tex_name, i);
        scene.textures.push(texture);
    }

    for material in &materials {
        let diffuse_color = [
            material.diffuse[0],
            material.diffuse[1],
            material.diffuse[2],
            material.diffuse[3],
        ];
        let emissive_color = [
            material.specular[0] * 0.1,
            material.specular[1] * 0.1,
            material.specular[2] * 0.1,
        ];

        let base_color_texture = if material.texture_index >= 0 {
            let idx = material.texture_index as usize;
            if idx < texture_count { Some(idx) } else { None }
        } else {
            None
        };

        let alpha_mode = if material.diffuse[3] < 1.0 {
            MaterialAlphaMode::Blend
        } else if material.draw_mode & 0x01 != 0 {
            MaterialAlphaMode::Mask
        } else {
            MaterialAlphaMode::Opaque
        };

        let sphere_texture = if material.sphere_mode_texture_index >= 0 {
            let idx = material.sphere_mode_texture_index as usize;
            if idx < texture_count { Some(idx) } else { None }
        } else {
            None
        };
        let toon_source = if material.toon_texture_index >= 0 {
            if material.toon_mode == PMXUtil::pmx_types::PMXToonModeRaw::Separate {
                let idx = material.toon_texture_index as usize;
                if idx < texture_count {
                    Some(MaterialToonSource::Separate(idx))
                } else {
                    None
                }
            } else {
                Some(MaterialToonSource::BuiltIn(
                    material.toon_texture_index as u8,
                ))
            }
        } else {
            None
        };

        scene.materials.push(MaterialCpu {
            base_color_factor: diffuse_color,
            base_color_texture,
            base_color_tex_coord: 0,
            base_color_uv_transform: None,
            base_color_wrap_s: TextureWrapMode::Repeat,
            base_color_wrap_t: TextureWrapMode::Repeat,
            base_color_min_filter: TextureFilterMode::Linear,
            base_color_mag_filter: TextureFilterMode::Linear,
            sphere_texture,
            toon_source,
            emissive_factor: emissive_color,
            alpha_mode,
            alpha_cutoff: 0.5,
            double_sided: material.draw_mode & 0x10 != 0,
        });
    }

    let mut warned_uv_morphs = HashSet::new();
    let mut warned_bone_morphs = HashSet::new();
    let mut warned_other_morphs = HashSet::new();
    let mut global_positions: Vec<Vec3> = Vec::with_capacity(vertices.len());
    let mut global_normals: Vec<Vec3> = Vec::with_capacity(vertices.len());
    let mut global_uvs: Vec<Vec2> = Vec::with_capacity(vertices.len());
    let mut global_joints4: Vec<[u16; 4]> = Vec::with_capacity(vertices.len());
    let mut global_weights4: Vec<[f32; 4]> = Vec::with_capacity(vertices.len());
    let mut global_sdef_vertices = Vec::with_capacity(vertices.len());

    for vertex in &vertices {
        global_positions.push(Vec3::new(
            vertex.position[0],
            vertex.position[1],
            vertex.position[2],
        ));
        global_normals.push(Vec3::new(vertex.norm[0], vertex.norm[1], vertex.norm[2]));
        global_uvs.push(Vec2::new(vertex.uv[0], vertex.uv[1]));
        let (j, w, sdef) = convert_vertex_weight(&vertex.weight_type);
        global_joints4.push(j);
        global_weights4.push(w);
        global_sdef_vertices.push(sdef);
    }

    let mut morph_stats = PmxMorphStats::default();
    for morph in &morphs {
        if morph.morph_data.is_empty() {
            morph_stats.empty += 1;
            continue;
        }
        match &morph.morph_data[0] {
            PMXUtil::pmx_types::MorphTypes::Vertex(_) => morph_stats.vertex += 1,
            PMXUtil::pmx_types::MorphTypes::UV(_)
            | PMXUtil::pmx_types::MorphTypes::UV1(_)
            | PMXUtil::pmx_types::MorphTypes::UV2(_)
            | PMXUtil::pmx_types::MorphTypes::UV3(_)
            | PMXUtil::pmx_types::MorphTypes::UV4(_) => morph_stats.uv += 1,
            PMXUtil::pmx_types::MorphTypes::Bone(_) => morph_stats.bone += 1,
            PMXUtil::pmx_types::MorphTypes::Material(_) => morph_stats.material += 1,
            PMXUtil::pmx_types::MorphTypes::Group(_)
            | PMXUtil::pmx_types::MorphTypes::Flip(_)
            | PMXUtil::pmx_types::MorphTypes::Impulse(_) => morph_stats.group_flip_impulse += 1,
        }
    }

    let mut face_offset = 0usize;
    for (mat_idx, material) in materials.iter().enumerate() {
        let face_count = (material.num_face_vertices as usize) / 3;
        if face_count == 0 {
            continue;
        }

        let result = build_mesh_for_material(
            mat_idx,
            face_offset,
            face_count,
            &faces,
            &morphs,
            &global_positions,
            &global_normals,
            &global_uvs,
            &global_joints4,
            &global_weights4,
            &global_sdef_vertices,
            &mut warned_uv_morphs,
            &mut warned_bone_morphs,
            &mut warned_other_morphs,
        );

        let mesh_index = scene.meshes.len();
        let morph_count = result.morph_count;
        scene.meshes.push(result.mesh);
        scene.mesh_instances.push(MeshInstance {
            mesh_index,
            node_index: 0,
            skin_index: if !scene.skins.is_empty() {
                Some(0)
            } else {
                None
            },
            default_morph_weights: vec![0.0; morph_count],
            layer: MeshLayer::Subject,
        });

        face_offset += face_count;
    }

    if scene.meshes.is_empty() {
        bail!("PMX has no renderable geometry: {}", path.display());
    }

    scene.material_morphs = extract_material_morphs(&morphs);
    let sdef_vertex_count = global_sdef_vertices
        .iter()
        .filter(|entry| entry.is_some())
        .count();
    pmx_log::info(format!(
        "PMX morph summary: total={}, vertex={}, uv={}, bone={}, material={}, group_flip_impulse={}, empty={}, extracted_material_morphs={}, sdef_vertices={}",
        morphs.len(),
        morph_stats.vertex,
        morph_stats.uv,
        morph_stats.bone,
        morph_stats.material,
        morph_stats.group_flip_impulse,
        morph_stats.empty,
        scene.material_morphs.len(),
        sdef_vertex_count
    ));
    scene.root_center_node = find_root_center_node(&scene.nodes);
    scene.pmx_rig_meta = Some(pmx_rig_meta);
    scene.pmx_physics_meta = Some(pmx_physics_meta);
    log_pmx_parity_report(&scene, &morph_stats);
    if let Some(profile) = derive_pmx_profile(&scene) {
        for line in profile.describe_lines() {
            pmx_log::info(line);
        }
    }
    Ok(scene)
}
