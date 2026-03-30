use crate::{
    engine::pmx_rig::{PmxJointKind, PmxPhysicsMeta, PmxRigMeta, PmxRigidCalcMethod},
    scene::SceneCpu,
};

use super::helpers::shape_bounding_radius;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PmxDerivedProfile {
    pub rig: PmxRigProfile,
    pub physics: PmxPhysicsProfile,
    pub solver: PmxSolverProfile,
}

impl PmxDerivedProfile {
    pub(crate) fn describe_lines(&self) -> [String; 3] {
        [
            format!(
                "PMX profile rig: bones={}, ik_bones={}, append_bones={}, fixed_axis_bones={}, local_axis_bones={}, external_parent_bones={}, ik_chains={}, avg_ik_iterations={:.2}, avg_ik_limit={:.3}",
                self.rig.bone_count,
                self.rig.ik_bone_count,
                self.rig.append_bone_count,
                self.rig.fixed_axis_bone_count,
                self.rig.local_axis_bone_count,
                self.rig.external_parent_bone_count,
                self.rig.ik_chain_count,
                self.rig.average_ik_iterations,
                self.rig.average_ik_limit_angle,
            ),
            format!(
                "PMX profile physics: bodies={}, static={}, dynamic={}, dynamic_with_bone={}, spring6dof={}, sixdof={}, p2p={}, cone_twist={}, slider={}, hinge={}, avg_move_resist={:.3}, avg_rotation_resist={:.3}, avg_repulsion={:.3}, avg_friction={:.3}, avg_mass={:.3}, avg_radius={:.3}, avg_spring_move={:.3}, avg_spring_rotation={:.3}, avg_joint_span={:.3}, floor_y={:.3}",
                self.physics.rigid_body_count,
                self.physics.static_body_count,
                self.physics.dynamic_body_count,
                self.physics.dynamic_with_bone_body_count,
                self.physics.spring_joint_count,
                self.physics.six_dof_joint_count,
                self.physics.p2p_joint_count,
                self.physics.cone_twist_joint_count,
                self.physics.slider_joint_count,
                self.physics.hinge_joint_count,
                self.physics.average_move_resist,
                self.physics.average_rotation_resist,
                self.physics.average_repulsion,
                self.physics.average_friction,
                self.physics.average_mass,
                self.physics.average_radius,
                self.physics.average_spring_move,
                self.physics.average_spring_rotation,
                self.physics.average_joint_span,
                self.physics.derived_floor_y,
            ),
            format!(
                "PMX solver profile: substep_dt={:.4}, max_substeps={}, joint_iterations={}, dynamic_follow={:.3}, dynamic_bone_follow={:.3}, spring_move={:.3}, spring_rotation={:.3}, joint_correction={:.3}, position_limit={:.3}, rotation_limit={:.3}, collision_push={:.3}",
                self.solver.target_substep_dt,
                self.solver.max_substeps,
                self.solver.joint_iterations,
                self.solver.dynamic_follow_gain,
                self.solver.dynamic_with_bone_follow_gain,
                self.solver.spring_move_gain,
                self.solver.spring_rotation_gain,
                self.solver.joint_correction_gain,
                self.solver.position_limit_gain,
                self.solver.rotation_limit_gain,
                self.solver.collision_push_gain,
            ),
        ]
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PmxRigProfile {
    pub bone_count: usize,
    pub ik_bone_count: usize,
    pub append_bone_count: usize,
    pub fixed_axis_bone_count: usize,
    pub local_axis_bone_count: usize,
    pub external_parent_bone_count: usize,
    pub ik_chain_count: usize,
    pub average_ik_iterations: f32,
    pub average_ik_limit_angle: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PmxPhysicsProfile {
    pub rigid_body_count: usize,
    pub static_body_count: usize,
    pub dynamic_body_count: usize,
    pub dynamic_with_bone_body_count: usize,
    pub spring_joint_count: usize,
    pub six_dof_joint_count: usize,
    pub p2p_joint_count: usize,
    pub cone_twist_joint_count: usize,
    pub slider_joint_count: usize,
    pub hinge_joint_count: usize,
    pub average_move_resist: f32,
    pub average_rotation_resist: f32,
    pub average_repulsion: f32,
    pub average_friction: f32,
    pub average_mass: f32,
    pub average_radius: f32,
    pub average_spring_move: f32,
    pub average_spring_rotation: f32,
    pub average_joint_span: f32,
    pub derived_floor_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PmxSolverProfile {
    pub target_substep_dt: f32,
    pub max_substeps: usize,
    pub joint_iterations: usize,
    pub dynamic_follow_gain: f32,
    pub dynamic_with_bone_follow_gain: f32,
    pub spring_move_gain: f32,
    pub spring_rotation_gain: f32,
    pub joint_correction_gain: f32,
    pub position_limit_gain: f32,
    pub rotation_limit_gain: f32,
    pub collision_push_gain: f32,
}

impl Default for PmxSolverProfile {
    fn default() -> Self {
        Self {
            target_substep_dt: 0.008,
            max_substeps: 8,
            joint_iterations: 4,
            dynamic_follow_gain: 0.12,
            dynamic_with_bone_follow_gain: 0.22,
            spring_move_gain: 0.18,
            spring_rotation_gain: 0.18,
            joint_correction_gain: 0.35,
            position_limit_gain: 1.0,
            rotation_limit_gain: 0.45,
            collision_push_gain: 0.30,
        }
    }
}

pub(crate) fn derive_pmx_profile(scene: &SceneCpu) -> Option<PmxDerivedProfile> {
    let rig = derive_rig_profile(scene.pmx_rig_meta.as_ref());
    let physics = derive_physics_profile(scene.pmx_physics_meta.as_ref());
    if rig.is_empty() && physics.is_empty() {
        return None;
    }

    let solver = derive_solver_profile(&rig, &physics);
    Some(PmxDerivedProfile {
        rig,
        physics,
        solver,
    })
}

impl PmxRigProfile {
    fn is_empty(&self) -> bool {
        self.bone_count == 0 && self.ik_chain_count == 0
    }
}

impl PmxPhysicsProfile {
    fn is_empty(&self) -> bool {
        self.rigid_body_count == 0 && self.spring_joint_count == 0 && self.six_dof_joint_count == 0
    }
}

fn derive_rig_profile(meta: Option<&PmxRigMeta>) -> PmxRigProfile {
    let Some(meta) = meta else {
        return PmxRigProfile::default();
    };

    let ik_chain_count = meta.ik_chains.len();
    let (total_iterations, total_limit_angle) =
        meta.ik_chains
            .iter()
            .fold((0.0_f32, 0.0_f32), |(iter_acc, limit_acc), chain| {
                (
                    iter_acc + chain.iterations as f32,
                    limit_acc + chain.limit_angle,
                )
            });
    let chain_count = ik_chain_count.max(1) as f32;

    PmxRigProfile {
        bone_count: meta.bones.len(),
        ik_bone_count: meta.count_bones_with_ik(),
        append_bone_count: meta.count_bones_with_append(),
        fixed_axis_bone_count: meta.count_bones_with_fixed_axis(),
        local_axis_bone_count: meta.count_bones_with_local_axis(),
        external_parent_bone_count: meta.count_bones_with_external_parent(),
        ik_chain_count,
        average_ik_iterations: total_iterations / chain_count,
        average_ik_limit_angle: total_limit_angle / chain_count,
    }
}

fn derive_physics_profile(meta: Option<&PmxPhysicsMeta>) -> PmxPhysicsProfile {
    let Some(meta) = meta else {
        return PmxPhysicsProfile::default();
    };

    let mut profile = PmxPhysicsProfile {
        rigid_body_count: meta.rigid_bodies.len(),
        static_body_count: 0,
        dynamic_body_count: 0,
        dynamic_with_bone_body_count: 0,
        spring_joint_count: 0,
        six_dof_joint_count: 0,
        p2p_joint_count: 0,
        cone_twist_joint_count: 0,
        slider_joint_count: 0,
        hinge_joint_count: 0,
        average_move_resist: 0.0,
        average_rotation_resist: 0.0,
        average_repulsion: 0.0,
        average_friction: 0.0,
        average_mass: 0.0,
        average_radius: 0.0,
        average_spring_move: 0.0,
        average_spring_rotation: 0.0,
        average_joint_span: 0.0,
        derived_floor_y: 0.0,
    };

    let rigid_count = profile.rigid_body_count.max(1) as f32;
    let mut floor_y = f32::INFINITY;
    for rigid in &meta.rigid_bodies {
        match rigid.calc_method {
            PmxRigidCalcMethod::Static => profile.static_body_count += 1,
            PmxRigidCalcMethod::Dynamic => profile.dynamic_body_count += 1,
            PmxRigidCalcMethod::DynamicWithBonePosition => {
                profile.dynamic_with_bone_body_count += 1
            }
        }
        profile.average_move_resist += rigid.move_resist.max(0.0);
        profile.average_rotation_resist += rigid.rotation_resist.max(0.0);
        profile.average_repulsion += rigid.repulsion.max(0.0);
        profile.average_friction += rigid.friction.max(0.0);
        profile.average_mass += rigid.mass.max(0.0);
        let radius = shape_bounding_radius(rigid.form, rigid.size);
        profile.average_radius += radius;
        floor_y = floor_y.min(rigid.position.y - radius);
    }
    profile.average_move_resist /= rigid_count;
    profile.average_rotation_resist /= rigid_count;
    profile.average_repulsion /= rigid_count;
    profile.average_friction /= rigid_count;
    profile.average_mass /= rigid_count;
    profile.average_radius /= rigid_count;
    profile.derived_floor_y = if floor_y.is_finite() { floor_y } else { 0.0 };

    let mut spring_move_count = 0.0_f32;
    let mut spring_rotation_count = 0.0_f32;
    let mut joint_span_count = 0.0_f32;
    for joint in &meta.joints {
        match &joint.kind {
            PmxJointKind::Spring6Dof {
                move_limit_down,
                move_limit_up,
                rotation_limit_down,
                rotation_limit_up,
                spring_const_move,
                spring_const_rotation,
                ..
            } => {
                profile.spring_joint_count += 1;
                profile.average_spring_move += spring_const_move.length();
                profile.average_spring_rotation += spring_const_rotation.length();
                spring_move_count += 1.0;
                spring_rotation_count += 1.0;
                joint_span_count += 1.0;
                profile.average_joint_span += (move_limit_up - move_limit_down).length()
                    + (rotation_limit_up - rotation_limit_down).length();
            }
            PmxJointKind::SixDof {
                move_limit_down,
                move_limit_up,
                rotation_limit_down,
                rotation_limit_up,
                ..
            } => {
                profile.six_dof_joint_count += 1;
                joint_span_count += 1.0;
                profile.average_joint_span += (move_limit_up - move_limit_down).length()
                    + (rotation_limit_up - rotation_limit_down).length();
            }
            PmxJointKind::P2P { .. } => {
                profile.p2p_joint_count += 1;
            }
            PmxJointKind::ConeTwist {
                swing_span1,
                swing_span2,
                twist_span,
                ..
            } => {
                profile.cone_twist_joint_count += 1;
                joint_span_count += 1.0;
                profile.average_joint_span +=
                    swing_span1.abs() + swing_span2.abs() + twist_span.abs();
            }
            PmxJointKind::Slider {
                lower_linear_limit,
                upper_linear_limit,
                lower_angle_limit,
                upper_angle_limit,
                ..
            } => {
                profile.slider_joint_count += 1;
                joint_span_count += 1.0;
                profile.average_joint_span += (upper_linear_limit - lower_linear_limit).abs()
                    + (upper_angle_limit - lower_angle_limit).abs();
            }
            PmxJointKind::Hinge { low, high, .. } => {
                profile.hinge_joint_count += 1;
                joint_span_count += 1.0;
                profile.average_joint_span += (high - low).abs();
            }
        }
    }
    if spring_move_count > 0.0 {
        profile.average_spring_move /= spring_move_count;
    }
    if spring_rotation_count > 0.0 {
        profile.average_spring_rotation /= spring_rotation_count;
    }
    if joint_span_count > 0.0 {
        profile.average_joint_span /= joint_span_count;
    }
    profile
}

fn derive_solver_profile(rig: &PmxRigProfile, physics: &PmxPhysicsProfile) -> PmxSolverProfile {
    if physics.is_empty() {
        return PmxSolverProfile::default();
    }

    let bone_count = rig.bone_count.max(1) as f32;
    let rigid_count = physics.rigid_body_count.max(1) as f32;
    let joint_count = (physics.spring_joint_count
        + physics.six_dof_joint_count
        + physics.p2p_joint_count
        + physics.cone_twist_joint_count
        + physics.slider_joint_count
        + physics.hinge_joint_count)
        .max(1) as f32;
    let spring_density = physics.spring_joint_count as f32 / rigid_count;
    let ik_density = rig.ik_chain_count as f32 / bone_count;

    let average_drag = (physics.average_move_resist + physics.average_rotation_resist) * 0.5;
    let average_spring = (physics.average_spring_move + physics.average_spring_rotation) * 0.5;
    let average_span = physics.average_joint_span.max(1e-4);
    let average_radius = physics.average_radius.max(1e-4);
    let average_mass = physics.average_mass.max(1e-4);
    let average_repulsion = physics.average_repulsion.max(0.0);

    let mut solver = PmxSolverProfile::default();
    solver.target_substep_dt = (1.0
        / (60.0
            + spring_density * 24.0
            + ik_density * 18.0
            + joint_count * 0.75
            + (physics.dynamic_with_bone_body_count as f32 / rigid_count) * 12.0))
        .clamp(0.004, 0.016);
    solver.max_substeps =
        (2 + physics.spring_joint_count / 16 + physics.dynamic_with_bone_body_count / 20)
            .clamp(2, 12);
    solver.joint_iterations = (2
        + physics.spring_joint_count / 20
        + rig.ik_chain_count / 4
        + (joint_count as usize / 24))
        .clamp(2, 10);
    solver.dynamic_follow_gain = (1.0 / (1.0 + average_drag)).clamp(0.05, 0.30);
    solver.dynamic_with_bone_follow_gain =
        (1.0 / (1.0 + physics.average_rotation_resist + average_radius * 0.25)).clamp(0.06, 0.40);
    solver.spring_move_gain = (1.0 / (1.0 + average_spring)).clamp(0.05, 0.45);
    solver.spring_rotation_gain =
        (1.0 / (1.0 + average_spring + average_span * 0.25)).clamp(0.05, 0.40);
    solver.joint_correction_gain =
        ((1.0 + average_repulsion) / (1.0 + average_mass + average_radius)).clamp(0.10, 0.60);
    solver.position_limit_gain = (0.85 + 0.15 / (1.0 + average_span * 0.25)).clamp(0.75, 1.0);
    solver.rotation_limit_gain =
        (0.70 + (rig.append_bone_count as f32 / bone_count) * 0.10 + ik_density * 0.10)
            .clamp(0.65, 1.0);
    solver.collision_push_gain =
        (0.12 + average_repulsion * 0.03 + average_radius * 0.06).clamp(0.10, 0.40);
    solver
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Node, SceneCpu};
    use glam::{Quat, Vec3};

    #[test]
    fn derive_pmx_profile_preserves_raw_counts_and_values() {
        let scene = SceneCpu {
            nodes: vec![Node {
                name: Some("root".to_owned()),
                name_en: None,
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            pmx_rig_meta: Some(PmxRigMeta {
                bones: vec![crate::engine::pmx_rig::PmxBoneMeta {
                    grant_transform: Some(crate::engine::pmx_rig::PmxGrantTransform {
                        parent_index: 0,
                        weight: 0.5,
                        is_local: false,
                        affects_rotation: true,
                        affects_translation: false,
                    }),
                    name: "ik".to_owned(),
                    name_en: "ik".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: -1,
                    deform_depth: 0,
                    boneflag: 0x0020 | 0x0100 | 0x0400 | 0x0800 | 0x2000,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: 0,
                    append_weight: 0.5,
                    fixed_axis: Vec3::X,
                    local_axis_x: Vec3::X,
                    local_axis_z: Vec3::Z,
                    key_value: 0,
                    ik_target_index: 0,
                    ik_iter_count: 8,
                    ik_limit: 0.25,
                }],
                ik_chains: vec![crate::engine::pmx_rig::IKChain {
                    controller_bone_index: 0,
                    target_bone_index: 0,
                    chain_root_bone_index: 0,
                    iterations: 8,
                    limit_angle: 0.25,
                    links: Vec::new(),
                }],
                grant_evaluation_order: vec![0],
                grant_cycle_bones: Vec::new(),
            }),
            pmx_physics_meta: Some(PmxPhysicsMeta {
                rigid_bodies: vec![crate::engine::pmx_rig::PmxRigidBodyCpu {
                    name: "rb".to_owned(),
                    name_en: "rb".to_owned(),
                    bone_index: 0,
                    group: 1,
                    un_collision_group_flag: 0,
                    form: crate::engine::pmx_rig::PmxRigidShape::Sphere,
                    size: Vec3::splat(0.5),
                    position: Vec3::new(0.0, 1.5, 0.0),
                    rotation: Vec3::ZERO,
                    mass: 2.0,
                    move_resist: 0.3,
                    rotation_resist: 0.4,
                    repulsion: 0.2,
                    friction: 0.1,
                    calc_method: PmxRigidCalcMethod::DynamicWithBonePosition,
                }],
                joints: vec![crate::engine::pmx_rig::PmxJointCpu {
                    name: "joint".to_owned(),
                    name_en: "joint".to_owned(),
                    kind: PmxJointKind::Spring6Dof {
                        a_rigid_index: 0,
                        b_rigid_index: 0,
                        position: Vec3::ZERO,
                        rotation: Vec3::ZERO,
                        move_limit_down: Vec3::splat(-0.1),
                        move_limit_up: Vec3::splat(0.1),
                        rotation_limit_down: Vec3::splat(-0.2),
                        rotation_limit_up: Vec3::splat(0.2),
                        spring_const_move: Vec3::splat(0.8),
                        spring_const_rotation: Vec3::splat(0.6),
                    },
                }],
            }),
            ..SceneCpu::default()
        };

        let profile = derive_pmx_profile(&scene).expect("profile");
        assert_eq!(profile.rig.bone_count, 1);
        assert_eq!(profile.rig.ik_bone_count, 1);
        assert_eq!(profile.rig.append_bone_count, 1);
        assert_eq!(profile.rig.fixed_axis_bone_count, 1);
        assert_eq!(profile.physics.rigid_body_count, 1);
        assert_eq!(profile.physics.dynamic_with_bone_body_count, 1);
        assert_eq!(profile.physics.spring_joint_count, 1);
        assert!(profile.physics.derived_floor_y < 1.5);
        assert!(profile.solver.target_substep_dt < 0.016);
        assert!(!profile.describe_lines()[0].is_empty());
    }
}
