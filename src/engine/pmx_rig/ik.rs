use glam::{Mat4, Quat, Vec3};

use crate::engine::pmx_rig::types::IKChain;

/// Solve a single IK chain using CCD (Cyclic Coordinate Descent).
///
/// This is a simplified CCD solver that rotates each joint in the chain
/// iteratively to minimize the distance between the effector and target.
///
/// # Arguments
/// * `chain` - The IK chain definition
/// * `nodes` - Skeleton nodes (for parent relationships)
/// * `poses` - Current pose (will be modified with IK results)
/// * `target_pos` - World-space target position for the effector
pub fn solve_ik_chain_ccd(
    chain: &IKChain,
    nodes: &[crate::scene::Node],
    poses: &mut [crate::scene::NodePose],
    target_pos: Vec3,
) {
    if chain.links.is_empty() {
        return;
    }

    // CCD iterates through joints from tip towards root
    // For each joint, find the rotation that minimizes effector-to-target distance
    for _iteration in 0..chain.iterations {
        // Iterate from the link closest to target (last in array) towards root
        for link_idx in (0..chain.links.len()).rev() {
            let link = &chain.links[link_idx];
            let joint_idx = link.bone_index;

            // Compute global position of the effector (target bone)
            let effector_global = compute_global_position(chain.target_bone_index, nodes, poses);

            // Compute global position of this joint
            let joint_global = compute_global_position(joint_idx, nodes, poses);

            // Vectors in world space
            let to_effector = effector_global - joint_global;
            let to_target = target_pos - joint_global;

            let to_effector_len = to_effector.length();
            let to_target_len = to_target.length();

            if to_effector_len < f32::EPSILON || to_target_len < f32::EPSILON {
                continue;
            }

            let to_effector_norm = to_effector / to_effector_len;
            let to_target_norm = to_target / to_target_len;

            // Rotation that aligns effector direction towards target direction
            let rotation = rotation_between(to_effector_norm, to_target_norm);

            let parent_rotation = nodes
                .get(joint_idx)
                .and_then(|node| node.parent)
                .map(|parent_index| {
                    let (_, rotation, _) = compute_global_transform(parent_index, nodes, poses)
                        .to_scale_rotation_translation();
                    rotation
                })
                .unwrap_or(Quat::IDENTITY);
            let local_rotation_delta =
                (parent_rotation.conjugate() * rotation * parent_rotation).normalize();

            // Apply rotation to this joint's local pose
            let current_rotation = poses[joint_idx].rotation;
            poses[joint_idx].rotation = (local_rotation_delta * current_rotation).normalize();

            // Apply angle limit if specified
            if let Some(limits) = &link.angle_limits {
                apply_angle_limits(&mut poses[joint_idx].rotation, limits, chain.limit_angle);
            }
        }
    }
}

/// Compute the global position of a bone given the current pose.
pub fn compute_bone_position(
    bone_index: usize,
    nodes: &[crate::scene::Node],
    poses: &[crate::scene::NodePose],
) -> Vec3 {
    compute_global_transform(bone_index, nodes, poses).transform_point3(Vec3::ZERO)
}

fn compute_global_position(
    bone_index: usize,
    nodes: &[crate::scene::Node],
    poses: &[crate::scene::NodePose],
) -> Vec3 {
    compute_bone_position(bone_index, nodes, poses)
}

fn compute_global_transform(
    bone_index: usize,
    nodes: &[crate::scene::Node],
    poses: &[crate::scene::NodePose],
) -> Mat4 {
    let mut transform = Mat4::IDENTITY;
    let mut current_idx = Some(bone_index);
    let mut visited = vec![false; nodes.len()];

    while let Some(idx) = current_idx {
        if idx >= poses.len() || idx >= nodes.len() || visited[idx] {
            break;
        }
        visited[idx] = true;

        let pose = &poses[idx];
        let local =
            Mat4::from_scale_rotation_translation(pose.scale, pose.rotation, pose.translation);
        transform = local * transform;
        current_idx = nodes[idx].parent;
    }

    transform
}

/// Create a rotation that rotates `from` direction to `to` direction.
pub(super) fn rotation_between(from: Vec3, to: Vec3) -> Quat {
    let dot = from.dot(to);
    if dot > 0.9999 {
        return Quat::IDENTITY;
    }
    if dot < -0.9999 {
        // Vectors are opposite, return a 180-degree rotation
        // Find an orthogonal axis
        let ortho = if from.x.abs() > from.y.abs() {
            Vec3::new(-from.z, 0.0, from.x).normalize()
        } else {
            Vec3::new(0.0, from.z, -from.y).normalize()
        };
        return Quat::from_rotation_arc(from, ortho) * Quat::from_rotation_arc(ortho, to);
    }

    Quat::from_rotation_arc(from, to)
}

/// Apply angle limits to a rotation, clamping each axis.
fn apply_angle_limits(rotation: &mut Quat, limits: &[Vec3; 2], max_angle: f32) {
    let (mut yaw, mut pitch, mut roll) = rotation.to_euler(glam::EulerRot::YXZ);

    let [min, max] = limits;
    yaw = yaw.clamp(min.x, max.x);
    pitch = pitch.clamp(min.y, max.y);
    roll = roll.clamp(min.z, max.z);

    yaw = yaw.clamp(-max_angle, max_angle);
    pitch = pitch.clamp(-max_angle, max_angle);
    roll = roll.clamp(-max_angle, max_angle);

    *rotation = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
}
