use glam::Vec3;

use crate::engine::skeleton::{compute_global_matrices, default_poses};
use crate::scene::SceneCpu;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CameraFraming {
    pub focus: Vec3,
    pub radius: f32,
    pub camera_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SceneStats {
    pub min: Vec3,
    pub max: Vec3,
    pub median: Vec3,
    pub p90_distance: f32,
    pub p98_distance: f32,
}

pub(crate) fn compute_scene_framing(
    scene: &SceneCpu,
    config_fov_deg: f32,
    user_orbit_radius: f32,
    user_camera_height: f32,
    user_look_at_y: f32,
) -> CameraFraming {
    let Some(stats) = scene_stats_world(scene) else {
        return CameraFraming {
            focus: Vec3::new(
                0.0,
                if user_look_at_y != 0.0 {
                    user_look_at_y
                } else {
                    1.0
                },
                0.0,
            ),
            radius: user_orbit_radius.max(0.1),
            camera_height: if user_camera_height != 0.0 {
                user_camera_height
            } else {
                1.2
            },
        };
    };

    let extent = (stats.max - stats.min).abs();
    let auto_focus_y = (stats.min.y + stats.max.y) * 0.5;
    let focus = Vec3::new(
        stats.median.x,
        if user_look_at_y != 0.0 {
            user_look_at_y
        } else {
            auto_focus_y
        },
        stats.median.z,
    );

    let fov_rad = config_fov_deg.to_radians().clamp(0.35, 2.6);
    let object_radius = stats
        .p98_distance
        .max(stats.p90_distance * 1.12)
        .max(extent.y * 0.52)
        .max(extent.x * 0.46)
        .max(0.25);
    let mut auto_radius = object_radius / (fov_rad * 0.5).tan();
    auto_radius = (auto_radius * 1.08).max(1.2);
    let auto_height = focus.y + extent.y.max(0.3) * 0.02;

    CameraFraming {
        focus,
        radius: if user_orbit_radius > 0.0 {
            user_orbit_radius
        } else {
            auto_radius
        },
        camera_height: if user_camera_height != 0.0 {
            user_camera_height
        } else {
            auto_height
        },
    }
}

pub(crate) fn scene_stats_world(scene: &SceneCpu) -> Option<SceneStats> {
    if scene.mesh_instances.is_empty() {
        return None;
    }
    let poses = default_poses(&scene.nodes);
    let globals = compute_global_matrices(&scene.nodes, &poses);

    let focus_mask = focus_node_mask(scene);
    let (mut min, mut max, mut points) =
        collect_scene_points(scene, &globals, focus_mask.as_deref());
    if points.is_empty() {
        (min, max, points) = collect_scene_points(scene, &globals, None);
    }
    if points.is_empty() {
        return None;
    }

    let mut xs = points.iter().map(|p| p.x).collect::<Vec<_>>();
    let mut ys = points.iter().map(|p| p.y).collect::<Vec<_>>();
    let mut zs = points.iter().map(|p| p.z).collect::<Vec<_>>();
    xs.sort_by(f32::total_cmp);
    ys.sort_by(f32::total_cmp);
    zs.sort_by(f32::total_cmp);

    let q01 = Vec3::new(
        quantile_sorted(&xs, 0.01),
        quantile_sorted(&ys, 0.01),
        quantile_sorted(&zs, 0.01),
    );
    let q99 = Vec3::new(
        quantile_sorted(&xs, 0.99),
        quantile_sorted(&ys, 0.99),
        quantile_sorted(&zs, 0.99),
    );
    let median = Vec3::new(
        quantile_sorted(&xs, 0.50),
        quantile_sorted(&ys, 0.50),
        quantile_sorted(&zs, 0.50),
    );

    let mut robust_min = q01;
    let mut robust_max = q99;
    if (robust_max - robust_min).abs().length_squared() < 1e-6 {
        robust_min = min;
        robust_max = max;
    }

    let mut distances = Vec::with_capacity(points.len());
    for p in &points {
        if p.x >= robust_min.x
            && p.x <= robust_max.x
            && p.y >= robust_min.y
            && p.y <= robust_max.y
            && p.z >= robust_min.z
            && p.z <= robust_max.z
        {
            distances.push((*p - median).length());
        }
    }
    if distances.is_empty() {
        distances.extend(points.iter().map(|p| (*p - median).length()));
    }
    distances.sort_by(f32::total_cmp);
    let p90_distance = quantile_sorted(&distances, 0.90).max(0.05);
    let p98_distance = quantile_sorted(&distances, 0.98).max(p90_distance);

    Some(SceneStats {
        min: robust_min,
        max: robust_max,
        median,
        p90_distance,
        p98_distance,
    })
}

pub(crate) fn focus_node_mask(scene: &SceneCpu) -> Option<Vec<bool>> {
    let root = scene.root_center_node?;
    if root >= scene.nodes.len() {
        return None;
    }
    let mut mask = vec![false; scene.nodes.len()];
    let mut stack = vec![root];
    while let Some(node_index) = stack.pop() {
        if node_index >= scene.nodes.len() || mask[node_index] {
            continue;
        }
        mask[node_index] = true;
        stack.extend(scene.nodes[node_index].children.iter().copied());
    }
    Some(mask)
}

pub(crate) fn collect_scene_points(
    scene: &SceneCpu,
    globals: &[glam::Mat4],
    focus_mask: Option<&[bool]>,
) -> (Vec3, Vec3, Vec<Vec3>) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut points = Vec::new();
    for instance in &scene.mesh_instances {
        if focus_mask
            .and_then(|mask| mask.get(instance.node_index))
            .copied()
            == Some(false)
        {
            continue;
        }
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let node_global = globals
            .get(instance.node_index)
            .copied()
            .unwrap_or(glam::Mat4::IDENTITY);
        for position in &mesh.positions {
            let p = node_global.transform_point3(*position);
            min = min.min(p);
            max = max.max(p);
            points.push(p);
        }
    }
    (min, max, points)
}

pub(crate) fn quantile_sorted(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let q = q.clamp(0.0, 1.0);
    let pos = q * ((sorted.len() - 1) as f32);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let t = pos - (lo as f32);
    sorted[lo] * (1.0 - t) + sorted[hi] * t
}
