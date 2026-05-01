use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::engine::{
    pmx_rig::{PmxJointKind, PmxRigidCalcMethod},
    skeleton::{compute_global_matrices_in_place, reset_poses_from_nodes},
};
use crate::scene::SceneCpu;

use super::{
    JointRuntime, JointRuntimeKind, PmxPhysicsState, RigidBodyRuntime, RuntimePmxSettings,
    derive_pmx_profile, helpers::shape_bounding_radius,
};

pub(super) fn from_scene(
    scene: &SceneCpu,
    settings: RuntimePmxSettings,
) -> Option<PmxPhysicsState> {
    let meta = scene.pmx_physics_meta.as_ref()?;
    if meta.is_empty() {
        return None;
    }

    let mut poses = Vec::new();
    reset_poses_from_nodes(&scene.nodes, &mut poses);
    let mut globals = Vec::new();
    let mut visited = Vec::new();
    compute_global_matrices_in_place(&scene.nodes, &poses, &mut globals, &mut visited);

    let bodies = meta
        .rigid_bodies
        .iter()
        .map(|rigid| {
            let rigid_rotation = Quat::from_euler(
                EulerRot::XYZ,
                rigid.rotation.x,
                rigid.rotation.y,
                rigid.rotation.z,
            );
            let bone_index = (rigid.bone_index >= 0).then_some(rigid.bone_index as usize);
            let (local_translation, local_rotation, target_world) = bone_index
                .and_then(|bone_index| {
                    globals
                        .get(bone_index)
                        .copied()
                        .map(|global| (bone_index, global))
                })
                .map(|(_bone_index, bone_global)| {
                    let (_, bone_rotation, bone_translation) =
                        bone_global.to_scale_rotation_translation();
                    let normalized_local_translation =
                        bone_rotation.conjugate() * (rigid.position - bone_translation);
                    let normalized_local_rotation =
                        (bone_rotation.conjugate() * rigid_rotation).normalize();
                    let world = bone_global
                        * Mat4::from_scale_rotation_translation(
                            Vec3::ONE,
                            normalized_local_rotation,
                            normalized_local_translation,
                        );
                    (
                        normalized_local_translation,
                        normalized_local_rotation,
                        world,
                    )
                })
                .unwrap_or_else(|| {
                    (
                        rigid.position,
                        rigid_rotation,
                        Mat4::from_scale_rotation_translation(
                            Vec3::ONE,
                            rigid_rotation,
                            rigid.position,
                        ),
                    )
                });
            let (_, rotation, position) = target_world.to_scale_rotation_translation();
            let inverse_mass = if matches!(rigid.calc_method, PmxRigidCalcMethod::Static)
                || rigid.mass <= f32::EPSILON
            {
                0.0
            } else {
                1.0 / rigid.mass.max(1e-4)
            };

            RigidBodyRuntime {
                bone_index,
                calc_method: rigid.calc_method,
                group: rigid.group,
                un_collision_group_flag: rigid.un_collision_group_flag,
                shape: rigid.form,
                size: rigid.size,
                local_translation,
                local_rotation,
                position,
                rotation,
                radius: shape_bounding_radius(rigid.form, rigid.size),
                linear_velocity: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                inverse_mass,
                linear_damping: rigid.move_resist.max(0.0),
                angular_damping: rigid.rotation_resist.max(0.0),
                repulsion: rigid.repulsion.max(0.0),
                friction: rigid.friction.max(0.0),
            }
        })
        .collect::<Vec<_>>();

    let mut joints = Vec::new();
    for joint in &meta.joints {
        let (
            kind,
            a_rigid_index,
            b_rigid_index,
            strength,
            spring_const_move,
            spring_const_rotation,
            anchor_position,
            move_limit_down,
            move_limit_up,
            rotation_limit_down,
            rotation_limit_up,
            lower_linear_limit,
            upper_linear_limit,
            lower_angle_limit,
            upper_angle_limit,
            cone_swing_span1,
            cone_swing_span2,
            cone_twist_span,
        ) = match &joint.kind {
            PmxJointKind::Spring6Dof {
                a_rigid_index,
                b_rigid_index,
                position,
                rotation: _,
                move_limit_down,
                move_limit_up,
                rotation_limit_down,
                rotation_limit_up,
                spring_const_move,
                spring_const_rotation,
                ..
            } => (
                JointRuntimeKind::Spring6Dof,
                *a_rigid_index,
                *b_rigid_index,
                0.18 + spring_const_move.length() * 0.02,
                Some(*spring_const_move),
                Some(*spring_const_rotation),
                Some(*position),
                Some(*move_limit_down),
                Some(*move_limit_up),
                Some(*rotation_limit_down),
                Some(*rotation_limit_up),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            PmxJointKind::SixDof {
                a_rigid_index,
                b_rigid_index,
                position,
                rotation: _,
                move_limit_down,
                move_limit_up,
                rotation_limit_down,
                rotation_limit_up,
                ..
            } => (
                JointRuntimeKind::SixDof,
                *a_rigid_index,
                *b_rigid_index,
                0.22,
                None,
                None,
                Some(*position),
                Some(*move_limit_down),
                Some(*move_limit_up),
                Some(*rotation_limit_down),
                Some(*rotation_limit_up),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            PmxJointKind::P2P {
                a_rigid_index,
                b_rigid_index,
                position,
                rotation: _,
                ..
            } => (
                JointRuntimeKind::P2P,
                *a_rigid_index,
                *b_rigid_index,
                0.22,
                None,
                None,
                Some(*position),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            PmxJointKind::ConeTwist {
                a_rigid_index,
                b_rigid_index,
                softness,
                bias_factor,
                relaxation_factor,
                damping,
                swing_span1,
                swing_span2,
                twist_span,
                ..
            } => (
                JointRuntimeKind::ConeTwist,
                *a_rigid_index,
                *b_rigid_index,
                0.12 + softness.abs() * 0.04
                    + bias_factor.abs() * 0.02
                    + relaxation_factor.abs() * 0.02
                    + damping.abs() * 0.02,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*swing_span1),
                Some(*swing_span2),
                Some(*twist_span),
                None,
            ),
            PmxJointKind::Slider {
                a_rigid_index,
                b_rigid_index,
                lower_linear_limit,
                upper_linear_limit,
                lower_angle_limit,
                upper_angle_limit,
                ..
            } => (
                JointRuntimeKind::Slider,
                *a_rigid_index,
                *b_rigid_index,
                0.16,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*lower_linear_limit),
                Some(*upper_linear_limit),
                Some(*lower_angle_limit),
                Some(*upper_angle_limit),
                None,
                None,
                None,
            ),
            PmxJointKind::Hinge {
                a_rigid_index,
                b_rigid_index,
                softness,
                bias_factor,
                relaxation_factor,
                low,
                high,
                ..
            } => (
                JointRuntimeKind::Hinge,
                *a_rigid_index,
                *b_rigid_index,
                0.12 + softness.abs() * 0.04
                    + bias_factor.abs() * 0.02
                    + relaxation_factor.abs() * 0.02,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*low),
                Some(*high),
                None,
                None,
                None,
            ),
        };

        if a_rigid_index < 0 || b_rigid_index < 0 {
            continue;
        }
        let Some(a) = bodies.get(a_rigid_index as usize) else {
            continue;
        };
        let Some(b) = bodies.get(b_rigid_index as usize) else {
            continue;
        };

        let joint_rotation = match &joint.kind {
            PmxJointKind::Spring6Dof { rotation, .. }
            | PmxJointKind::SixDof { rotation, .. }
            | PmxJointKind::P2P { rotation, .. } => {
                Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z)
            }
            PmxJointKind::ConeTwist { .. }
            | PmxJointKind::Slider { .. }
            | PmxJointKind::Hinge { .. } => Quat::IDENTITY,
        };

        let (a_anchor_local, b_anchor_local) = match anchor_position {
            Some(anchor) => {
                let a_local = a.rotation.conjugate() * (anchor - a.position);
                let b_local = b.rotation.conjugate() * (anchor - b.position);
                (Some(a_local), Some(b_local))
            }
            None => (None, None),
        };

        joints.push(JointRuntime {
            kind,
            a_rigid_index: a_rigid_index as usize,
            b_rigid_index: b_rigid_index as usize,
            joint_position: anchor_position.unwrap_or(Vec3::ZERO),
            joint_rotation,
            rest_offset: b.position - a.position,
            strength,
            spring_const_move,
            spring_const_rotation,
            a_anchor_local,
            b_anchor_local,
            move_limit_down,
            move_limit_up,
            rotation_limit_down,
            rotation_limit_up,
            lower_linear_limit,
            upper_linear_limit,
            lower_angle_limit,
            upper_angle_limit,
            cone_swing_span1,
            cone_swing_span2,
            cone_twist_span,
        });
    }

    let profile = derive_pmx_profile(scene)?;

    Some(PmxPhysicsState {
        settings,
        bodies,
        joints,
        profile,
    })
}
