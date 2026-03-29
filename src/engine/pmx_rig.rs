//! PMX-specific rig metadata for IK and advanced skinning.
//!
//! This module stores PMX-specific bone metadata that doesn't fit into the
//! generic `SceneCpu`/`Node` structures, specifically IK chain definitions.

use glam::{Mat4, Quat, Vec3};

/// A single link in an IK chain.
#[derive(Debug, Clone)]
pub struct IKLink {
    /// Bone index in the skeleton.
    pub bone_index: usize,
    /// Optional angle limits (min, max) for each axis in radians.
    /// PMX uses axis-angle limits, approximated as per-axis limits.
    pub angle_limits: Option<[Vec3; 2]>,
}

/// An IK chain definition from PMX.
#[derive(Debug, Clone)]
pub struct IKChain {
    pub controller_bone_index: usize,
    /// The effector bone (end point) that IK tries to reach.
    pub target_bone_index: usize,
    /// Root of the IK chain (usually the first link).
    /// The solver iterates from here towards the target.
    pub chain_root_bone_index: usize,
    /// Maximum iterations for the solver.
    pub iterations: u32,
    /// Angle limit in radians per iteration step.
    pub limit_angle: f32,
    /// Links in the chain (from root towards target).
    pub links: Vec<IKLink>,
}

/// PMX-specific rig metadata extracted from the model.
#[derive(Debug, Clone, Default)]
pub struct PmxRigMeta {
    /// All IK chains defined in the model.
    pub ik_chains: Vec<IKChain>,
}

impl PmxRigMeta {
    /// Returns true if there are no IK chains.
    pub fn is_empty(&self) -> bool {
        self.ik_chains.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmxRigidShape {
    Sphere,
    Box,
    Capsule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmxRigidCalcMethod {
    Static,
    Dynamic,
    DynamicWithBonePosition,
}

#[derive(Debug, Clone)]
pub struct PmxRigidBodyCpu {
    pub name: String,
    pub name_en: String,
    pub bone_index: i32,
    pub group: u8,
    pub un_collision_group_flag: u16,
    pub form: PmxRigidShape,
    pub size: Vec3,
    pub position: Vec3,
    pub rotation: Vec3,
    pub mass: f32,
    pub move_resist: f32,
    pub rotation_resist: f32,
    pub repulsion: f32,
    pub friction: f32,
    pub calc_method: PmxRigidCalcMethod,
}

#[derive(Debug, Clone)]
pub enum PmxJointKind {
    Spring6Dof {
        a_rigid_index: i32,
        b_rigid_index: i32,
        position: Vec3,
        rotation: Vec3,
        move_limit_down: Vec3,
        move_limit_up: Vec3,
        rotation_limit_down: Vec3,
        rotation_limit_up: Vec3,
        spring_const_move: Vec3,
        spring_const_rotation: Vec3,
    },
    SixDof {
        a_rigid_index: i32,
        b_rigid_index: i32,
        position: Vec3,
        rotation: Vec3,
        move_limit_down: Vec3,
        move_limit_up: Vec3,
        rotation_limit_down: Vec3,
        rotation_limit_up: Vec3,
    },
    P2P {
        a_rigid_index: i32,
        b_rigid_index: i32,
        position: Vec3,
        rotation: Vec3,
    },
    ConeTwist {
        a_rigid_index: i32,
        b_rigid_index: i32,
        swing_span1: f32,
        swing_span2: f32,
        twist_span: f32,
        softness: f32,
        bias_factor: f32,
        relaxation_factor: f32,
        damping: f32,
        fix_thresh: f32,
        enable_motor: bool,
        max_motor_impulse: f32,
        motor_target_in_constraint_space: Vec3,
    },
    Slider {
        a_rigid_index: i32,
        b_rigid_index: i32,
        lower_linear_limit: f32,
        upper_linear_limit: f32,
        lower_angle_limit: f32,
        upper_angle_limit: f32,
        power_linear_motor: bool,
        target_linear_motor_velocity: f32,
        max_linear_motor_force: f32,
        power_angler_motor: bool,
        target_angler_motor_velocity: f32,
        max_angler_motor_force: f32,
    },
    Hinge {
        a_rigid_index: i32,
        b_rigid_index: i32,
        low: f32,
        high: f32,
        softness: f32,
        bias_factor: f32,
        relaxation_factor: f32,
        enable_motor: bool,
        target_velocity: f32,
        max_motor_impulse: f32,
    },
}

#[derive(Debug, Clone)]
pub struct PmxJointCpu {
    pub name: String,
    pub name_en: String,
    pub kind: PmxJointKind,
}

#[derive(Debug, Clone, Default)]
pub struct PmxPhysicsMeta {
    pub rigid_bodies: Vec<PmxRigidBodyCpu>,
    pub joints: Vec<PmxJointCpu>,
}

impl PmxPhysicsMeta {
    pub fn is_empty(&self) -> bool {
        self.rigid_bodies.is_empty() && self.joints.is_empty()
    }
}

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

            // Apply rotation to this joint's pose
            let current_rotation = poses[joint_idx].rotation;
            poses[joint_idx].rotation = (rotation * current_rotation).normalize();

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

    transform.transform_point3(Vec3::ZERO)
}

fn compute_global_position(
    bone_index: usize,
    nodes: &[crate::scene::Node],
    poses: &[crate::scene::NodePose],
) -> Vec3 {
    compute_bone_position(bone_index, nodes, poses)
}

/// Create a rotation that rotates `from` direction to `to` direction.
fn rotation_between(from: Vec3, to: Vec3) -> Quat {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_rig_meta() {
        let meta = PmxRigMeta::default();
        assert!(meta.is_empty());
        assert!(meta.ik_chains.is_empty());
    }

    #[test]
    fn test_ik_chain_creation() {
        let chain = IKChain {
            controller_bone_index: 1,
            target_bone_index: 5,
            chain_root_bone_index: 2,
            iterations: 10,
            limit_angle: 0.1,
            links: vec![
                IKLink {
                    bone_index: 2,
                    angle_limits: None,
                },
                IKLink {
                    bone_index: 3,
                    angle_limits: None,
                },
                IKLink {
                    bone_index: 4,
                    angle_limits: None,
                },
            ],
        };
        assert_eq!(chain.controller_bone_index, 1);
        assert_eq!(chain.target_bone_index, 5);
        assert_eq!(chain.links.len(), 3);
    }

    #[test]
    fn test_rotation_identity_near_aligned() {
        let from = Vec3::X;
        let to = Vec3::X * 0.9999 + Vec3::Y * 0.01;
        let rot = rotation_between(from.normalize(), to.normalize());
        let result = rot * from.normalize();
        assert!((result - to.normalize()).length() < 0.02);
    }

    #[test]
    fn test_compute_bone_position_uses_pose_translation_and_parent_rotation() {
        let nodes = vec![
            crate::scene::Node {
                name: Some("root".to_owned()),
                parent: None,
                children: vec![1],
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
            crate::scene::Node {
                name: Some("child".to_owned()),
                parent: Some(0),
                children: Vec::new(),
                base_translation: Vec3::new(1.0, 0.0, 0.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
        ];

        let poses = vec![
            crate::scene::NodePose {
                translation: Vec3::new(10.0, 0.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                scale: Vec3::ONE,
            },
            crate::scene::NodePose {
                translation: Vec3::new(1.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ];

        let position = compute_bone_position(1, &nodes, &poses);
        assert!((position - Vec3::new(10.0, 1.0, 0.0)).length() < 1e-5);
    }
}
