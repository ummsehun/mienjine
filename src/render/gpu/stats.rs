use glam::{Mat4, Vec3, Vec4};

use crate::renderer::{Camera, PixelFrame, RenderStats};
use crate::scene::{MaterialAlphaMode, MeshLayer, RenderConfig, SceneCpu, StageRole};

pub(super) fn compute_gpu_render_stats(
    pixel_frame: &PixelFrame,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let mut stats = RenderStats::default();
    let width = pixel_frame.width_px as usize;
    let height = pixel_frame.height_px as usize;
    if width == 0 || height == 0 {
        return stats;
    }

    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    if let Some((x, y, depth)) = project_root_screen_gpu(
        scene,
        global_matrices,
        model_rotation,
        config,
        camera,
        pixel_frame.width_px,
        pixel_frame.height_px,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }

    if let Some(subject) = project_subject_metrics_gpu(
        scene,
        global_matrices,
        skin_matrices,
        instance_morph_weights,
        model_rotation,
        config,
        camera,
        pixel_frame.width_px,
        pixel_frame.height_px,
    ) {
        stats.subject_visible_ratio = subject.visible_ratio;
        stats.subject_visible_height_ratio = subject.height_ratio;
        stats.subject_centroid_px = Some(subject.centroid);
        stats.subject_bbox_px = Some(subject.bbox);
    }

    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let alpha = pixel_frame.rgba8.get(idx + 3).copied().unwrap_or(0);
            if alpha == 0 {
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

    let total = width.saturating_mul(height).max(1);
    stats.visible_cell_ratio = (visible as f32) / (total as f32);
    stats.visible_centroid_px = stats.root_screen_px;
    stats.visible_bbox_px = None;
    stats.visible_bbox_aspect = 0.0;
    stats.visible_height_ratio = 0.0;
    if visible > 0 {
        if stats.visible_centroid_px.is_none() {
            stats.visible_centroid_px = Some((sum_x / visible as f32, sum_y / visible as f32));
        }
        let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
        let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
        stats.visible_bbox_px = Some((
            min_x as u16,
            min_y as u16,
            max_x.min(width.saturating_sub(1)) as u16,
            max_y.min(height.saturating_sub(1)) as u16,
        ));
        stats.visible_bbox_aspect = if bbox_h > f32::EPSILON {
            bbox_w / bbox_h
        } else {
            0.0
        };
        stats.visible_height_ratio = (bbox_h / height as f32).clamp(0.0, 1.0);
    }

    stats.triangles_total = count_gpu_triangles(scene, config);
    stats.pixels_drawn = visible;
    stats
}

fn project_root_screen_gpu(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    model_rotation: Mat4,
    config: &RenderConfig,
    camera: Camera,
    width: u32,
    height: u32,
) -> Option<(f32, f32, f32)> {
    let node_index = scene.root_center_node?;
    let global = global_matrices
        .get(node_index)
        .copied()
        .unwrap_or(Mat4::IDENTITY);
    let world = (model_rotation * global).transform_point3(glam::Vec3::ZERO);
    let aspect = ((width as f32) * config.cell_aspect).max(1.0) / (height as f32).max(1.0);
    let projection =
        crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let clip = projection * view * world.extend(1.0);
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

fn count_gpu_triangles(scene: &SceneCpu, config: &RenderConfig) -> usize {
    let mut total = 0usize;
    for instance in &scene.mesh_instances {
        if matches!(instance.layer, MeshLayer::Stage) && matches!(config.stage_role, StageRole::Off)
        {
            continue;
        }
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let alpha_mode = mesh
            .material_index
            .and_then(|material_index| scene.materials.get(material_index))
            .map(|material| material.alpha_mode)
            .unwrap_or(MaterialAlphaMode::Opaque);
        if matches!(instance.layer, MeshLayer::Stage)
            && matches!(alpha_mode, MaterialAlphaMode::Blend)
        {
            continue;
        }
        total = total.saturating_add(mesh.indices.len());
    }
    total
}

struct SubjectMetrics {
    centroid: (f32, f32),
    bbox: (u16, u16, u16, u16),
    visible_ratio: f32,
    height_ratio: f32,
}

fn project_subject_metrics_gpu(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    model_rotation: Mat4,
    config: &RenderConfig,
    camera: Camera,
    width: u32,
    height: u32,
) -> Option<SubjectMetrics> {
    let aspect = ((width as f32) * config.cell_aspect).max(1.0) / (height as f32).max(1.0);
    let projection =
        crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;

    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width as usize;
    let mut min_y = height as usize;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        if !matches!(instance.layer, MeshLayer::Subject) {
            continue;
        }
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let node_global = global_matrices
            .get(instance.node_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        let model = model_rotation * node_global;
        let morph_weights = instance_morph_weights
            .get(instance_index)
            .map(|v| v.as_slice());
        let skin = instance.skin_index.and_then(|idx| skin_matrices.get(idx));

        for (vertex_index, position) in mesh.positions.iter().enumerate() {
            let mut pos = *position;
            if let Some(weights) = morph_weights {
                pos = apply_morph_position(mesh, vertex_index, pos, weights);
            }
            pos = apply_skin_position(mesh, vertex_index, pos, skin);

            let world = model.transform_point3(pos);
            let clip = view_projection * world.extend(1.0);
            if clip.w <= 1e-5 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.z < -1.0 || ndc.z > 1.0 {
                continue;
            }

            let x = (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0);
            let y = (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }

            visible = visible.saturating_add(1);
            sum_x += x;
            sum_y += y;
            let px = x.clamp(0.0, (width.saturating_sub(1)) as f32).floor() as usize;
            let py = y.clamp(0.0, (height.saturating_sub(1)) as f32).floor() as usize;
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        }
    }

    if visible == 0 {
        return None;
    }

    let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
    let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
    let frame_area = (width as f32).max(1.0) * (height as f32).max(1.0);
    Some(SubjectMetrics {
        centroid: (sum_x / visible as f32, sum_y / visible as f32),
        bbox: (
            min_x as u16,
            min_y as u16,
            max_x.min(width.saturating_sub(1) as usize) as u16,
            max_y.min(height.saturating_sub(1) as usize) as u16,
        ),
        visible_ratio: (bbox_w * bbox_h / frame_area).clamp(0.0, 1.0),
        height_ratio: (bbox_h / (height as f32)).clamp(0.0, 1.0),
    })
}

fn apply_morph_position(
    mesh: &crate::scene::MeshCpu,
    vertex_index: usize,
    base_position: Vec3,
    weights: &[f32],
) -> Vec3 {
    if mesh.morph_targets.is_empty() || weights.is_empty() {
        return base_position;
    }
    let mut out = base_position;
    for (target_index, target) in mesh.morph_targets.iter().enumerate() {
        let weight = weights.get(target_index).copied().unwrap_or(0.0);
        if weight.abs() <= 1e-5 {
            continue;
        }
        if let Some(delta) = target.position_deltas.get(vertex_index) {
            out += *delta * weight;
        }
    }
    out
}

fn apply_skin_position(
    mesh: &crate::scene::MeshCpu,
    vertex_index: usize,
    position: Vec3,
    skin_matrices: Option<&Vec<Mat4>>,
) -> Vec3 {
    let Some(joints) = mesh.joints4.as_ref() else {
        return position;
    };
    let Some(weights) = mesh.weights4.as_ref() else {
        return position;
    };
    let Some(skin_matrices) = skin_matrices else {
        return position;
    };

    let Some(joints) = joints.get(vertex_index) else {
        return position;
    };
    let Some(weights) = weights.get(vertex_index) else {
        return position;
    };

    let mut skinned = Vec4::ZERO;
    let mut accumulated = 0.0;
    for i in 0..4 {
        let weight = weights[i];
        if weight <= 0.0 {
            continue;
        }
        let Some(joint_matrix) = skin_matrices.get(joints[i] as usize) else {
            continue;
        };
        skinned += (*joint_matrix * position.extend(1.0)) * weight;
        accumulated += weight;
    }
    if accumulated <= f32::EPSILON {
        return position;
    }
    if skinned.w.abs() > 1e-6 {
        skinned.truncate() / skinned.w
    } else {
        skinned.truncate()
    }
}
