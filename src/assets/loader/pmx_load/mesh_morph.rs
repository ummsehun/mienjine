use std::collections::HashSet;
use std::path::{Path, PathBuf};

use glam::{Vec2, Vec3};

use crate::scene::{MeshCpu, MorphTargetCpu, SdefVertexCpu};
use crate::shared::pmx_log;

pub(crate) fn resolve_pmx_texture_path(model_path: &Path, raw_texture_path: &str) -> PathBuf {
    let base_dir = model_path.parent().unwrap_or_else(|| Path::new("."));
    let normalized = raw_texture_path.replace('\\', "/");
    let trimmed = normalized
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned();

    let mut candidates: Vec<PathBuf> = Vec::new();
    let push_unique = |path: PathBuf, list: &mut Vec<PathBuf>| {
        if !list.iter().any(|existing| existing == &path) {
            list.push(path);
        }
    };

    let normalized_path = PathBuf::from(&normalized);
    if normalized_path.is_absolute() {
        push_unique(normalized_path, &mut candidates);
    }

    let raw_path = PathBuf::from(raw_texture_path);
    if raw_path.is_absolute() {
        push_unique(raw_path, &mut candidates);
    }

    push_unique(base_dir.join(&normalized), &mut candidates);
    if !trimmed.is_empty() {
        push_unique(base_dir.join(trimmed), &mut candidates);
    }
    push_unique(base_dir.join(raw_texture_path), &mut candidates);

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| base_dir.join(normalized))
}

pub(super) struct MeshBuildResult {
    pub mesh: MeshCpu,
    pub morph_count: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_mesh_for_material(
    mat_idx: usize,
    face_offset: usize,
    face_count: usize,
    faces: &[PMXUtil::pmx_types::PMXFace],
    morphs: &[PMXUtil::pmx_types::PMXMorph],
    global_positions: &[Vec3],
    global_normals: &[Vec3],
    global_uvs: &[Vec2],
    global_joints4: &[[u16; 4]],
    global_weights4: &[[f32; 4]],
    global_sdef_vertices: &[Option<SdefVertexCpu>],
    warned_uv_morphs: &mut HashSet<String>,
    warned_bone_morphs: &mut HashSet<String>,
    warned_other_morphs: &mut HashSet<String>,
) -> MeshBuildResult {
    let mut used_verts: Vec<usize> = Vec::new();
    let mut vert_map: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut indices: Vec<u32> = Vec::with_capacity(face_count * 3);

    for fi in 0..face_count {
        let face_idx = face_offset + fi;
        if face_idx >= faces.len() {
            break;
        }
        let face = &faces[face_idx];
        for &vi in &face.vertices {
            let vi = vi as usize;
            let local_idx = *vert_map.entry(vi).or_insert_with(|| {
                let idx = used_verts.len() as u32;
                used_verts.push(vi);
                idx
            });
            indices.push(local_idx);
        }
    }

    let positions: Vec<Vec3> = used_verts.iter().map(|&vi| global_positions[vi]).collect();
    let normals: Vec<Vec3> = used_verts.iter().map(|&vi| global_normals[vi]).collect();
    let uvs: Vec<Vec2> = used_verts.iter().map(|&vi| global_uvs[vi]).collect();
    let joints4: Vec<[u16; 4]> = used_verts.iter().map(|&vi| global_joints4[vi]).collect();
    let weights4: Vec<[f32; 4]> = used_verts.iter().map(|&vi| global_weights4[vi]).collect();
    let sdef_vertices = if used_verts
        .iter()
        .any(|&vi| global_sdef_vertices[vi].is_some())
    {
        Some(
            used_verts
                .iter()
                .map(|&vi| global_sdef_vertices[vi])
                .collect(),
        )
    } else {
        None
    };

    let indices_chunked: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();

    let mut morphs_for_mesh: Vec<MorphTargetCpu> = Vec::new();
    for morph in morphs {
        if morph.morph_data.is_empty() {
            continue;
        }
        match &morph.morph_data[0] {
            PMXUtil::pmx_types::MorphTypes::Vertex(_) => {
                let mut position_deltas = vec![Vec3::ZERO; used_verts.len()];
                let normal_deltas = vec![Vec3::ZERO; used_verts.len()];
                for morph_data in &morph.morph_data {
                    if let PMXUtil::pmx_types::MorphTypes::Vertex(vm) = morph_data {
                        let vi = vm.index as usize;
                        if let Some(&local_idx) = vert_map.get(&vi) {
                            let local_idx = local_idx as usize;
                            if local_idx < position_deltas.len() {
                                position_deltas[local_idx] =
                                    Vec3::new(vm.offset[0], vm.offset[1], vm.offset[2]);
                            }
                        }
                    }
                }
                morphs_for_mesh.push(MorphTargetCpu {
                    name: Some(morph.name.clone()),
                    position_deltas,
                    normal_deltas,
                    uv0_deltas: None,
                    uv1_deltas: None,
                });
            }
            PMXUtil::pmx_types::MorphTypes::UV(_)
            | PMXUtil::pmx_types::MorphTypes::UV1(_)
            | PMXUtil::pmx_types::MorphTypes::UV2(_)
            | PMXUtil::pmx_types::MorphTypes::UV3(_)
            | PMXUtil::pmx_types::MorphTypes::UV4(_) => {
                let mut uv0_deltas = None;
                let mut uv1_deltas = None;
                match &morph.morph_data[0] {
                    PMXUtil::pmx_types::MorphTypes::UV(_) => {
                        let mut deltas = vec![Vec2::ZERO; used_verts.len()];
                        for morph_data in &morph.morph_data {
                            if let PMXUtil::pmx_types::MorphTypes::UV(vm) = morph_data {
                                let vi = vm.index as usize;
                                if let Some(&local_idx) = vert_map.get(&vi) {
                                    let local_idx = local_idx as usize;
                                    if local_idx < deltas.len() {
                                        deltas[local_idx] = Vec2::new(vm.offset[0], vm.offset[1]);
                                    }
                                }
                            }
                        }
                        uv0_deltas = Some(deltas);
                    }
                    PMXUtil::pmx_types::MorphTypes::UV1(_) => {
                        let mut deltas = vec![Vec2::ZERO; used_verts.len()];
                        for morph_data in &morph.morph_data {
                            if let PMXUtil::pmx_types::MorphTypes::UV1(vm) = morph_data {
                                let vi = vm.index as usize;
                                if let Some(&local_idx) = vert_map.get(&vi) {
                                    let local_idx = local_idx as usize;
                                    if local_idx < deltas.len() {
                                        deltas[local_idx] = Vec2::new(vm.offset[0], vm.offset[1]);
                                    }
                                }
                            }
                        }
                        uv1_deltas = Some(deltas);
                    }
                    PMXUtil::pmx_types::MorphTypes::UV2(_)
                    | PMXUtil::pmx_types::MorphTypes::UV3(_)
                    | PMXUtil::pmx_types::MorphTypes::UV4(_)
                        if warned_uv_morphs.insert(morph.name.clone()) =>
                    {
                        pmx_log::warn(format!(
                            "warning: UV2/UV3/UV4 morph '{}' are not mapped to the terminal renderer; only UV and UV1 are applied.",
                            &morph.name
                        ));
                    }
                    _ => {}
                }
                morphs_for_mesh.push(MorphTargetCpu {
                    name: Some(morph.name.clone()),
                    position_deltas: vec![Vec3::ZERO; used_verts.len()],
                    normal_deltas: vec![Vec3::ZERO; used_verts.len()],
                    uv0_deltas,
                    uv1_deltas,
                });
            }
            PMXUtil::pmx_types::MorphTypes::Bone(_) => {
                if warned_bone_morphs.insert(morph.name.clone()) {
                    pmx_log::warn(format!(
                        "warning: Bone morph '{}' not supported in MVP.",
                        &morph.name
                    ));
                }
            }
            PMXUtil::pmx_types::MorphTypes::Material(_) => {}
            PMXUtil::pmx_types::MorphTypes::Group(_)
            | PMXUtil::pmx_types::MorphTypes::Flip(_)
            | PMXUtil::pmx_types::MorphTypes::Impulse(_) => {
                if warned_other_morphs.insert(morph.name.clone()) {
                    pmx_log::warn(format!(
                        "warning: Morph '{}' type not supported in MVP.",
                        &morph.name
                    ));
                }
            }
        }
    }

    let morph_count = morphs_for_mesh.len();
    let mesh = MeshCpu {
        positions,
        normals,
        uv0: Some(uvs),
        uv1: None,
        colors_rgba: None,
        material_index: Some(mat_idx),
        indices: indices_chunked,
        joints4: Some(joints4),
        weights4: Some(weights4),
        sdef_vertices,
        morph_targets: morphs_for_mesh,
    };

    MeshBuildResult { mesh, morph_count }
}
