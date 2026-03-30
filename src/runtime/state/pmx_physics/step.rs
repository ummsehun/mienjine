use glam::{Mat4, Vec3};

use crate::engine::pmx_rig::PmxRigidCalcMethod;
use crate::scene::{NodePose, SceneCpu};

use super::{helpers, PmxPhysicsState};

pub(super) fn step(
    state: &mut PmxPhysicsState,
    scene: &SceneCpu,
    poses: &mut [NodePose],
    pre_physics_globals: &[Mat4],
    dt: f32,
) {
    let solver = state.profile.solver;
    let physics = state.profile.physics;
    let dt = dt.clamp(
        0.0,
        state.settings.unit_step * state.settings.max_substeps.max(1) as f32,
    );
    if dt <= f32::EPSILON || state.bodies.is_empty() {
        return;
    }

    let target_substep_dt = state.settings.unit_step.max(0.0005);
    let substeps =
        ((dt / target_substep_dt).ceil() as usize).clamp(1, state.settings.max_substeps.max(1));
    let sub_dt = dt / substeps as f32;

    for _ in 0..substeps {
        for body in &mut state.bodies {
            let (target_pos, target_rot) =
                helpers::target_body_transform(scene, pre_physics_globals, body);
            match body.calc_method {
                PmxRigidCalcMethod::Static => {
                    body.position = target_pos;
                    body.rotation = target_rot;
                    body.linear_velocity = Vec3::ZERO;
                    body.angular_velocity = Vec3::ZERO;
                }
                PmxRigidCalcMethod::Dynamic => {
                    let resist = body.linear_damping.clamp(0.0, 0.995);
                    let body_response = (1.0 + physics.average_repulsion)
                        / (1.0 + physics.average_mass + physics.average_radius.max(0.000_1));
                    let follow = (solver.dynamic_follow_gain * (1.0 - resist) * body_response)
                        .clamp(0.01, 0.35);
                    let stiffness =
                        (body.repulsion.max(0.0) + physics.average_repulsion) * follow + follow;
                    let damping = (body.linear_damping
                        + physics.average_rotation_resist * 0.5
                        + physics.average_move_resist * 0.5)
                        .clamp(0.0, 10.0);
                    let friction_damping =
                        (body.friction + physics.average_friction).clamp(0.0, 2.0) * 0.5;
                    body.linear_velocity += ((target_pos - body.position) * stiffness
                        + state.settings.gravity)
                        * sub_dt;
                    body.linear_velocity *=
                        (1.0 / (1.0 + (damping + friction_damping) * sub_dt)).clamp(0.0, 1.0);
                    let max_speed = (body.radius.max(0.05)
                        * (12.0 + solver.collision_push_gain * 20.0))
                        .clamp(0.5, 24.0);
                    if body.linear_velocity.length() > max_speed {
                        body.linear_velocity = body.linear_velocity.normalize() * max_speed;
                    }
                    body.position += body.linear_velocity * sub_dt;

                    let rot_alpha = (1.0
                        - (-((body.angular_damping * solver.rotation_limit_gain)
                            + solver.rotation_limit_gain)
                            * sub_dt)
                            .exp())
                    .clamp(0.0, 1.0);
                    body.rotation = body.rotation.slerp(target_rot, rot_alpha).normalize();
                }
                PmxRigidCalcMethod::DynamicWithBonePosition => {
                    body.position = target_pos;
                    body.linear_velocity = Vec3::ZERO;
                    body.angular_velocity = Vec3::ZERO;
                    let rot_alpha = (1.0
                        - (-((body.angular_damping * solver.rotation_limit_gain)
                            + solver.dynamic_with_bone_follow_gain)
                            * sub_dt)
                            .exp())
                    .clamp(0.0, 1.0);
                    body.rotation = body.rotation.slerp(target_rot, rot_alpha).normalize();
                }
            }
        }

        let iterations = if state.joints.is_empty() {
            0
        } else {
            solver.joint_iterations
        };
        for _ in 0..iterations {
            for joint in &state.joints {
                if joint.a_rigid_index >= state.bodies.len()
                    || joint.b_rigid_index >= state.bodies.len()
                {
                    continue;
                }
                if joint.a_rigid_index == joint.b_rigid_index {
                    continue;
                }

                let (a_idx, b_idx) = if joint.a_rigid_index < joint.b_rigid_index {
                    (joint.a_rigid_index, joint.b_rigid_index)
                } else {
                    (joint.b_rigid_index, joint.a_rigid_index)
                };
                let (left, right) = state.bodies.split_at_mut(b_idx);
                let (body_a, body_b) = if joint.a_rigid_index < joint.b_rigid_index {
                    (&mut left[a_idx], &mut right[0])
                } else {
                    (&mut right[0], &mut left[a_idx])
                };

                let error = if let (Some(a_anchor_local), Some(b_anchor_local)) =
                    (joint.a_anchor_local, joint.b_anchor_local)
                {
                    let a_anchor_world = body_a.position + body_a.rotation * a_anchor_local;
                    let b_anchor_world = body_b.position + body_b.rotation * b_anchor_local;
                    b_anchor_world - a_anchor_world
                } else {
                    body_b.position - body_a.position - joint.rest_offset
                };
                let strength = match joint.kind {
                    super::JointRuntimeKind::Spring6Dof => {
                        let move_strength = joint
                            .spring_const_move
                            .map(|spring| {
                                (spring.length() * solver.spring_move_gain).clamp(0.0, 0.9)
                            })
                            .unwrap_or_else(|| joint.strength.clamp(0.0, 0.9));
                        let rotation_strength = joint
                            .spring_const_rotation
                            .map(|spring| {
                                (spring.length() * solver.spring_rotation_gain).clamp(0.0, 0.9)
                            })
                            .unwrap_or(move_strength);
                        move_strength.max(rotation_strength)
                    }
                    _ => (joint.strength * solver.joint_correction_gain).clamp(0.0, 0.8),
                };
                let correction = error * strength;
                let correction_len = correction.length();
                let joint_correction_scale = match joint.kind {
                    super::JointRuntimeKind::Spring6Dof => 0.55,
                    super::JointRuntimeKind::SixDof => 0.45,
                    super::JointRuntimeKind::P2P => 0.35,
                    super::JointRuntimeKind::ConeTwist => 0.28,
                    super::JointRuntimeKind::Slider => 0.24,
                    super::JointRuntimeKind::Hinge => 0.20,
                };
                let max_correction = (body_a.radius + body_b.radius)
                    * (joint_correction_scale + strength * solver.joint_correction_gain)
                    + 0.01;
                let correction = if correction_len > max_correction && correction_len > f32::EPSILON
                {
                    correction * (max_correction / correction_len)
                } else {
                    correction
                };
                let inv_a = body_a.inverse_mass;
                let inv_b = body_b.inverse_mass;
                let total = inv_a + inv_b;
                if total > f32::EPSILON {
                    body_a.position += correction * (inv_a / total);
                    body_b.position -= correction * (inv_b / total);
                }

                helpers::apply_joint_limits(
                    joint,
                    body_a,
                    body_b,
                    strength,
                    solver.position_limit_gain,
                    solver.rotation_limit_gain,
                );
            }
        }

        for i in 0..state.bodies.len() {
            for j in (i + 1)..state.bodies.len() {
                let (body_a, body_b) = {
                    let (left, right) = state.bodies.split_at_mut(j);
                    (&mut left[i], &mut right[0])
                };
                if state.joints.iter().any(|joint| {
                    (joint.a_rigid_index == i && joint.b_rigid_index == j)
                        || (joint.a_rigid_index == j && joint.b_rigid_index == i)
                }) {
                    continue;
                }
                if !helpers::collision_pair_enabled(
                    body_a.group,
                    body_a.un_collision_group_flag,
                    body_b.group,
                ) || !helpers::collision_pair_enabled(
                    body_b.group,
                    body_b.un_collision_group_flag,
                    body_a.group,
                ) {
                    continue;
                }
                if body_a.bone_index.is_some() && body_b.bone_index == body_a.bone_index {
                    continue;
                }

                let delta = body_b.position - body_a.position;
                let distance = delta.length();
                let broad_min_distance = body_a.radius + body_b.radius;
                if distance <= f32::EPSILON || distance >= broad_min_distance {
                    continue;
                }
                let dir = delta / distance;
                let min_distance = helpers::shape_support_radius(body_a, dir)
                    + helpers::shape_support_radius(body_b, -dir);
                if distance >= min_distance {
                    continue;
                }
                let push = (min_distance - distance) * 0.5;
                let inv_a = body_a.inverse_mass;
                let inv_b = body_b.inverse_mass;
                let total = inv_a + inv_b;
                if total <= f32::EPSILON {
                    continue;
                }
                let push = (push * solver.collision_push_gain)
                    .min((body_a.radius + body_b.radius) * solver.collision_push_gain);
                body_a.position -= dir * push * (inv_a / total);
                body_b.position += dir * push * (inv_b / total);
            }
        }

        for body in &mut state.bodies {
            if matches!(body.calc_method, PmxRigidCalcMethod::Static) {
                continue;
            }
            if matches!(
                body.calc_method,
                PmxRigidCalcMethod::DynamicWithBonePosition
            ) {
                let (target_pos, target_rot) =
                    helpers::target_body_transform(scene, pre_physics_globals, body);
                body.position = target_pos;
                body.linear_velocity = Vec3::ZERO;
                body.rotation = body.rotation.slerp(target_rot, 0.45).normalize();
            }
            let floor_y = physics.derived_floor_y;
            if body.position.y < floor_y {
                body.position.y = floor_y;
                if body.linear_velocity.y < 0.0 {
                    body.linear_velocity.y = 0.0;
                }
            }
        }
    }

    let mut body_bone_globals = vec![None; scene.nodes.len()];
    for body in &state.bodies {
        let Some(bone_index) = body.bone_index else {
            continue;
        };
        if bone_index >= body_bone_globals.len()
            || matches!(body.calc_method, PmxRigidCalcMethod::Static)
        {
            continue;
        }
        if matches!(
            body.calc_method,
            PmxRigidCalcMethod::DynamicWithBonePosition
        ) && bone_is_in_physics_conflicting_ik_chain(scene, bone_index)
        {
            continue;
        }

        let bone_rotation = body.rotation * body.local_rotation.conjugate();
        let bone_translation = match body.calc_method {
            PmxRigidCalcMethod::DynamicWithBonePosition => pre_physics_globals
                .get(bone_index)
                .map(|global| global.transform_point3(Vec3::ZERO))
                .unwrap_or(body.position - bone_rotation * body.local_translation),
            PmxRigidCalcMethod::Dynamic => body.position - bone_rotation * body.local_translation,
            PmxRigidCalcMethod::Static => continue,
        };
        body_bone_globals[bone_index] = Some(Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            bone_rotation,
            bone_translation,
        ));
    }

    for body in &state.bodies {
        let Some(bone_index) = body.bone_index else {
            continue;
        };
        if bone_index >= poses.len() || matches!(body.calc_method, PmxRigidCalcMethod::Static) {
            continue;
        }

        let Some(bone_global) = body_bone_globals.get(bone_index).and_then(|v| *v) else {
            continue;
        };
        let parent_global = scene
            .nodes
            .get(bone_index)
            .and_then(|node| node.parent)
            .and_then(|parent_index| {
                body_bone_globals
                    .get(parent_index)
                    .and_then(|value| *value)
                    .or_else(|| pre_physics_globals.get(parent_index).copied())
            })
            .unwrap_or(Mat4::IDENTITY);
        let local = parent_global.inverse() * bone_global;
        let (scale, rotation, translation) = local.to_scale_rotation_translation();
        poses[bone_index].translation = translation;
        poses[bone_index].rotation = rotation;
        poses[bone_index].scale = scale;
    }
}

fn bone_is_in_physics_conflicting_ik_chain(scene: &SceneCpu, bone_index: usize) -> bool {
    let Some(rig_meta) = scene.pmx_rig_meta.as_ref() else {
        return false;
    };
    let Some(physics_meta) = scene.pmx_physics_meta.as_ref() else {
        return false;
    };

    rig_meta.ik_chains.iter().any(|chain| {
        let chain_contains_bone = chain.target_bone_index == bone_index
            || chain.controller_bone_index == bone_index
            || chain.links.iter().any(|link| link.bone_index == bone_index);
        if !chain_contains_bone {
            return false;
        }
        chain.links.iter().any(|link| {
            physics_meta.rigid_bodies.iter().any(|rigid| {
                rigid.bone_index >= 0
                    && rigid.bone_index as usize == link.bone_index
                    && !matches!(rigid.calc_method, PmxRigidCalcMethod::Static)
            })
        })
    })
}
