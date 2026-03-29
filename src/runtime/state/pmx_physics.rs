use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::engine::{
    animation::{compute_global_matrices_in_place, reset_poses_from_nodes},
    pmx_rig::{PmxJointKind, PmxRigidCalcMethod},
};
use crate::scene::{NodePose, SceneCpu};

#[derive(Debug, Clone)]
pub(crate) struct PmxPhysicsState {
    gravity: Vec3,
    bodies: Vec<RigidBodyRuntime>,
    joints: Vec<JointRuntime>,
}

#[derive(Debug, Clone)]
struct RigidBodyRuntime {
    bone_index: Option<usize>,
    calc_method: PmxRigidCalcMethod,
    group: u8,
    un_collision_group_flag: u16,
    local_translation: Vec3,
    local_rotation: Quat,
    position: Vec3,
    rotation: Quat,
    radius: f32,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
    inverse_mass: f32,
    linear_damping: f32,
    angular_damping: f32,
    repulsion: f32,
}

#[derive(Debug, Clone)]
struct JointRuntime {
    a_rigid_index: usize,
    b_rigid_index: usize,
    rest_offset: Vec3,
    strength: f32,
    a_anchor_local: Option<Vec3>,
    b_anchor_local: Option<Vec3>,
}

impl PmxPhysicsState {
    pub(crate) fn from_scene(scene: &SceneCpu) -> Option<Self> {
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
                let local_rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    rigid.rotation.x,
                    rigid.rotation.y,
                    rigid.rotation.z,
                );
                let local_translation = rigid.position;
                let target_world = rigid
                    .bone_index
                    .try_into()
                    .ok()
                    .and_then(|bone_index: usize| globals.get(bone_index).copied())
                    .map(|bone_global| {
                        bone_global
                            * Mat4::from_scale_rotation_translation(
                                Vec3::ONE,
                                local_rotation,
                                local_translation,
                            )
                    })
                    .unwrap_or_else(|| {
                        Mat4::from_scale_rotation_translation(
                            Vec3::ONE,
                            local_rotation,
                            local_translation,
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
                    bone_index: (rigid.bone_index >= 0).then_some(rigid.bone_index as usize),
                    calc_method: rigid.calc_method.clone(),
                    group: rigid.group,
                    un_collision_group_flag: rigid.un_collision_group_flag,
                    local_translation,
                    local_rotation,
                    position,
                    rotation,
                    radius: rigid.size.max_element().max(0.01),
                    linear_velocity: Vec3::ZERO,
                    angular_velocity: Vec3::ZERO,
                    inverse_mass,
                    linear_damping: rigid.move_resist.max(0.0),
                    angular_damping: rigid.rotation_resist.max(0.0),
                    repulsion: rigid.repulsion.max(0.0),
                }
            })
            .collect::<Vec<_>>();

        let mut joints = Vec::new();
        for joint in &meta.joints {
            let (a_rigid_index, b_rigid_index, strength, anchor_position) = match &joint.kind {
                PmxJointKind::Spring6Dof {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    spring_const_move,
                    ..
                } => (
                    *a_rigid_index,
                    *b_rigid_index,
                    0.18 + spring_const_move.length() * 0.02,
                    Some(*position),
                ),
                PmxJointKind::SixDof {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    ..
                }
                | PmxJointKind::P2P {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    ..
                } => (*a_rigid_index, *b_rigid_index, 0.22, Some(*position)),
                PmxJointKind::ConeTwist {
                    a_rigid_index,
                    b_rigid_index,
                    softness,
                    bias_factor,
                    relaxation_factor,
                    damping,
                    ..
                } => (
                    *a_rigid_index,
                    *b_rigid_index,
                    0.12 + softness.abs() * 0.04
                        + bias_factor.abs() * 0.02
                        + relaxation_factor.abs() * 0.02
                        + damping.abs() * 0.02,
                    None,
                ),
                PmxJointKind::Slider {
                    a_rigid_index,
                    b_rigid_index,
                    ..
                } => (*a_rigid_index, *b_rigid_index, 0.16, None),
                PmxJointKind::Hinge {
                    a_rigid_index,
                    b_rigid_index,
                    softness,
                    bias_factor,
                    relaxation_factor,
                    ..
                } => (
                    *a_rigid_index,
                    *b_rigid_index,
                    0.12 + softness.abs() * 0.04
                        + bias_factor.abs() * 0.02
                        + relaxation_factor.abs() * 0.02,
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

            let (a_anchor_local, b_anchor_local) = anchor_position
                .map(|anchor| {
                    let a_local = a.rotation.conjugate() * (anchor - a.position);
                    let b_local = b.rotation.conjugate() * (anchor - b.position);
                    (Some(a_local), Some(b_local))
                })
                .unwrap_or((None, None));

            joints.push(JointRuntime {
                a_rigid_index: a_rigid_index as usize,
                b_rigid_index: b_rigid_index as usize,
                rest_offset: b.position - a.position,
                strength,
                a_anchor_local,
                b_anchor_local,
            });
        }

        Some(Self {
            gravity: Vec3::new(0.0, -9.8 * 0.45, 0.0),
            bodies,
            joints,
        })
    }

    pub(crate) fn step(
        &mut self,
        scene: &SceneCpu,
        poses: &mut [NodePose],
        pre_physics_globals: &[Mat4],
        dt: f32,
    ) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= f32::EPSILON || self.bodies.is_empty() {
            return;
        }

        for body in &mut self.bodies {
            let (target_pos, target_rot) = target_body_transform(scene, pre_physics_globals, body);
            match body.calc_method {
                PmxRigidCalcMethod::Static => {
                    body.position = target_pos;
                    body.rotation = target_rot;
                    body.linear_velocity = Vec3::ZERO;
                    body.angular_velocity = Vec3::ZERO;
                }
                PmxRigidCalcMethod::Dynamic | PmxRigidCalcMethod::DynamicWithBonePosition => {
                    let follow = match body.calc_method {
                        PmxRigidCalcMethod::Dynamic => 0.42,
                        PmxRigidCalcMethod::DynamicWithBonePosition => 0.82,
                        PmxRigidCalcMethod::Static => 0.0,
                    };
                    let stiffness = (10.0 + body.repulsion * 6.0) * follow;
                    let damping = (body.linear_damping * 0.65 + 0.35).clamp(0.0, 8.0);
                    body.linear_velocity +=
                        ((target_pos - body.position) * stiffness + self.gravity) * dt;
                    body.linear_velocity -= body.linear_velocity * damping * dt;
                    body.position += body.linear_velocity * dt;

                    let rot_alpha =
                        (1.0 - (-((body.angular_damping * 0.8) + 0.6) * dt).exp()).clamp(0.0, 1.0);
                    body.rotation = body.rotation.slerp(target_rot, rot_alpha).normalize();
                }
            }
        }

        let iterations = if self.joints.is_empty() { 0 } else { 2 };
        for _ in 0..iterations {
            for joint in &self.joints {
                if joint.a_rigid_index >= self.bodies.len()
                    || joint.b_rigid_index >= self.bodies.len()
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
                let (left, right) = self.bodies.split_at_mut(b_idx);
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
                let strength = (joint.strength * dt).clamp(0.0, 0.35);
                let correction = error * strength;
                let inv_a = body_a.inverse_mass;
                let inv_b = body_b.inverse_mass;
                let total = inv_a + inv_b;
                if total > f32::EPSILON {
                    body_a.position += correction * (inv_a / total);
                    body_b.position -= correction * (inv_b / total);
                }
            }
        }

        for i in 0..self.bodies.len() {
            for j in (i + 1)..self.bodies.len() {
                let (body_a, body_b) = {
                    let (left, right) = self.bodies.split_at_mut(j);
                    (&mut left[i], &mut right[0])
                };
                if !collision_pair_enabled(
                    body_a.group,
                    body_a.un_collision_group_flag,
                    body_b.group,
                ) || !collision_pair_enabled(
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
                let min_distance = body_a.radius + body_b.radius;
                if distance <= f32::EPSILON || distance >= min_distance {
                    continue;
                }
                let push = (min_distance - distance) * 0.5;
                let dir = delta / distance;
                let inv_a = body_a.inverse_mass;
                let inv_b = body_b.inverse_mass;
                let total = inv_a + inv_b;
                if total <= f32::EPSILON {
                    continue;
                }
                body_a.position -= dir * push * (inv_a / total);
                body_b.position += dir * push * (inv_b / total);
            }
        }

        let mut body_bone_globals = vec![None; scene.nodes.len()];
        for body in &self.bodies {
            let Some(bone_index) = body.bone_index else {
                continue;
            };
            if bone_index >= body_bone_globals.len()
                || matches!(body.calc_method, PmxRigidCalcMethod::Static)
            {
                continue;
            }

            let body_world =
                Mat4::from_scale_rotation_translation(Vec3::ONE, body.rotation, body.position);
            let body_bind = Mat4::from_scale_rotation_translation(
                Vec3::ONE,
                body.local_rotation,
                body.local_translation,
            );
            body_bone_globals[bone_index] = Some(body_world * body_bind.inverse());
        }

        for body in &self.bodies {
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
}

fn target_body_transform(
    scene: &SceneCpu,
    pre_physics_globals: &[Mat4],
    body: &RigidBodyRuntime,
) -> (Vec3, Quat) {
    let local = Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        body.local_rotation,
        body.local_translation,
    );
    let target = body
        .bone_index
        .and_then(|bone_index| pre_physics_globals.get(bone_index).copied())
        .map(|bone_global| bone_global * local)
        .unwrap_or(local);
    let (_, rotation, translation) = target.to_scale_rotation_translation();
    if body.bone_index.is_some() && scene.nodes.is_empty() {
        (body.position, body.rotation)
    } else {
        (translation, rotation)
    }
}

fn collision_pair_enabled(group: u8, mask: u16, other_group: u8) -> bool {
    let bit = 1u16.checked_shl(other_group as u32).unwrap_or(0);
    (mask & bit) == 0 && group < 16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pmx_rig::{
        PmxJointCpu, PmxPhysicsMeta, PmxRigidBodyCpu, PmxRigidCalcMethod, PmxRigidShape,
    };
    use crate::scene::{Node, SceneCpu};
    use glam::Quat;

    #[test]
    fn dynamic_body_moves_under_gravity() {
        let scene = SceneCpu {
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            pmx_physics_meta: Some(PmxPhysicsMeta {
                rigid_bodies: vec![PmxRigidBodyCpu {
                    name: "rb".to_owned(),
                    name_en: "rb".to_owned(),
                    bone_index: -1,
                    group: 0,
                    un_collision_group_flag: 0,
                    form: PmxRigidShape::Sphere,
                    size: Vec3::splat(0.1),
                    position: Vec3::new(0.0, 1.0, 0.0),
                    rotation: Vec3::ZERO,
                    mass: 1.0,
                    move_resist: 0.0,
                    rotation_resist: 0.0,
                    repulsion: 0.0,
                    friction: 0.0,
                    calc_method: PmxRigidCalcMethod::Dynamic,
                }],
                joints: Vec::<PmxJointCpu>::new(),
            }),
            ..SceneCpu::default()
        };
        let mut state = PmxPhysicsState::from_scene(&scene).expect("physics state");
        let mut poses = vec![NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }];
        let mut globals = Vec::new();
        let mut visited = Vec::new();
        compute_global_matrices_in_place(&scene.nodes, &poses, &mut globals, &mut visited);

        state.step(&scene, &mut poses, &globals, 0.2);

        assert!(state.bodies[0].position.y < 1.0);
    }

    #[test]
    fn static_body_follows_bone_target() {
        let scene = SceneCpu {
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::new(1.0, 2.0, 3.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            pmx_physics_meta: Some(PmxPhysicsMeta {
                rigid_bodies: vec![PmxRigidBodyCpu {
                    name: "rb".to_owned(),
                    name_en: "rb".to_owned(),
                    bone_index: 0,
                    group: 0,
                    un_collision_group_flag: 0,
                    form: PmxRigidShape::Sphere,
                    size: Vec3::splat(0.1),
                    position: Vec3::ZERO,
                    rotation: Vec3::ZERO,
                    mass: 1.0,
                    move_resist: 0.0,
                    rotation_resist: 0.0,
                    repulsion: 0.0,
                    friction: 0.0,
                    calc_method: PmxRigidCalcMethod::Static,
                }],
                joints: Vec::new(),
            }),
            ..SceneCpu::default()
        };
        let state = PmxPhysicsState::from_scene(&scene).expect("physics state");
        assert!((state.bodies[0].position - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn dynamic_body_backprojects_to_bone_space() {
        let scene = SceneCpu {
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            pmx_physics_meta: Some(PmxPhysicsMeta {
                rigid_bodies: vec![PmxRigidBodyCpu {
                    name: "rb".to_owned(),
                    name_en: "rb".to_owned(),
                    bone_index: 0,
                    group: 0,
                    un_collision_group_flag: 0,
                    form: PmxRigidShape::Sphere,
                    size: Vec3::splat(0.1),
                    position: Vec3::new(0.0, 1.0, 0.0),
                    rotation: Vec3::ZERO,
                    mass: 1.0,
                    move_resist: 0.0,
                    rotation_resist: 0.0,
                    repulsion: 0.0,
                    friction: 0.0,
                    calc_method: PmxRigidCalcMethod::Dynamic,
                }],
                joints: Vec::new(),
            }),
            ..SceneCpu::default()
        };
        let mut state = PmxPhysicsState::from_scene(&scene).expect("physics state");
        state.bodies[0].position = Vec3::new(0.0, 2.0, 0.0);
        state.bodies[0].rotation = Quat::IDENTITY;
        let mut poses = vec![NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }];
        let globals = vec![Mat4::IDENTITY];

        state.step(&scene, &mut poses, &globals, 0.000001);

        assert!((poses[0].translation.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn collision_mask_skips_blocked_pairs() {
        let scene = SceneCpu {
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            pmx_physics_meta: Some(PmxPhysicsMeta {
                rigid_bodies: vec![
                    PmxRigidBodyCpu {
                        name: "a".to_owned(),
                        name_en: "a".to_owned(),
                        bone_index: -1,
                        group: 0,
                        un_collision_group_flag: 0b10,
                        form: PmxRigidShape::Sphere,
                        size: Vec3::splat(0.5),
                        position: Vec3::ZERO,
                        rotation: Vec3::ZERO,
                        mass: 1.0,
                        move_resist: 0.0,
                        rotation_resist: 0.0,
                        repulsion: 0.0,
                        friction: 0.0,
                        calc_method: PmxRigidCalcMethod::Dynamic,
                    },
                    PmxRigidBodyCpu {
                        name: "b".to_owned(),
                        name_en: "b".to_owned(),
                        bone_index: -1,
                        group: 1,
                        un_collision_group_flag: 0,
                        form: PmxRigidShape::Sphere,
                        size: Vec3::splat(0.5),
                        position: Vec3::new(0.4, 0.0, 0.0),
                        rotation: Vec3::ZERO,
                        mass: 1.0,
                        move_resist: 0.0,
                        rotation_resist: 0.0,
                        repulsion: 0.0,
                        friction: 0.0,
                        calc_method: PmxRigidCalcMethod::Dynamic,
                    },
                ],
                joints: Vec::new(),
            }),
            ..SceneCpu::default()
        };
        let mut state = PmxPhysicsState::from_scene(&scene).expect("physics state");
        let mut poses = vec![NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }];
        let globals = vec![Mat4::IDENTITY];

        state.step(&scene, &mut poses, &globals, 0.01);

        assert!((state.bodies[1].position.x - 0.4).abs() < 1e-4);
    }
}
