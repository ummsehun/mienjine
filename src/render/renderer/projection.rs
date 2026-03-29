//! Projection, skinning, and morph target helpers.

use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::scene::{MeshCpu, SceneCpu};

use super::ProjectedVertex;

pub(super) fn project_root_screen(
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

pub(super) fn apply_skin(
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

pub(super) fn apply_morph_targets(
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

pub(super) fn project_mesh_vertices(
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
        let screen = glam::Vec2::new(
            (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0),
            (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0),
        );
        let depth = (ndc.z + 1.0) * 0.5;
        let uv0 = mesh
            .uv0
            .as_ref()
            .and_then(|values| values.get(index).copied())
            .unwrap_or(glam::Vec2::ZERO);
        let uv1 = mesh
            .uv1
            .as_ref()
            .and_then(|values| values.get(index).copied())
            .unwrap_or(uv0);
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
            uv1,
            vertex_color,
            material_index: mesh.material_index,
        });
    }
}
