use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::engine::animation::compute_global_matrices_in_place;
use crate::engine::pmx_rig::{
    PmxJointCpu, PmxJointKind, PmxPhysicsMeta, PmxRigidBodyCpu, PmxRigidCalcMethod, PmxRigidShape,
};
use crate::runtime::state::RuntimePmxSettings;
use crate::scene::{Node, NodePose, SceneCpu};

use super::{helpers, PmxPhysicsState, RigidBodyRuntime};

fn single_root_scene(rigid_bodies: Vec<PmxRigidBodyCpu>) -> SceneCpu {
    SceneCpu {
        nodes: vec![Node {
            name: Some("root".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        pmx_physics_meta: Some(PmxPhysicsMeta {
            rigid_bodies,
            joints: Vec::new(),
        }),
        ..SceneCpu::default()
    }
}

#[test]
fn dynamic_body_moves_under_gravity() {
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
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
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
            name_en: None,
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
                position: Vec3::new(1.0, 2.0, 3.0),
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
    let state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    assert!((state.bodies[0].position - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
}

#[test]
fn dynamic_body_backprojects_to_bone_space() {
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
        pmx_physics_meta: Some(PmxPhysicsMeta {
            rigid_bodies: vec![PmxRigidBodyCpu {
                name: "rb".to_owned(),
                name_en: "rb".to_owned(),
                bone_index: 0,
                group: 0,
                un_collision_group_flag: 0,
                form: PmxRigidShape::Sphere,
                size: Vec3::splat(0.1),
                position: Vec3::new(0.25, 1.0, -0.5),
                rotation: Vec3::new(0.0, 0.35, 0.0),
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
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    state.bodies[0].position = Vec3::new(1.5, 2.5, -0.25);
    state.bodies[0].rotation = Quat::from_rotation_y(0.5);
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.000001);

    let expected_rotation = state.bodies[0].rotation * state.bodies[0].local_rotation.conjugate();
    let expected_translation =
        state.bodies[0].position - expected_rotation * state.bodies[0].local_translation;
    assert!((poses[0].translation - expected_translation).length() < 1e-4);
    assert!((poses[0].rotation - expected_rotation).length() < 1e-4);
}

#[test]
fn dynamic_with_bone_position_follows_source_bone_more_tightly() {
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
                calc_method: PmxRigidCalcMethod::DynamicWithBonePosition,
            }],
            joints: Vec::new(),
        }),
        ..SceneCpu::default()
    };

    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    state.bodies[0].position = Vec3::new(0.0, 3.0, 0.0);
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.016);

    let target_pos = Vec3::new(0.0, 1.0, 0.0);
    assert!((state.bodies[0].position - target_pos).length() < 2.0);
}

#[test]
fn dynamic_with_bone_position_preserves_bone_translation_on_writeback() {
    let scene = SceneCpu {
        nodes: vec![Node {
            name: Some("root".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::new(0.0, 1.0, 0.0),
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
                calc_method: PmxRigidCalcMethod::DynamicWithBonePosition,
            }],
            joints: Vec::new(),
        }),
        ..SceneCpu::default()
    };

    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    state.bodies[0].position = Vec3::new(5.0, 7.0, -2.0);
    state.bodies[0].rotation = Quat::from_rotation_y(0.5);
    let mut poses = vec![NodePose {
        translation: scene.nodes[0].base_translation,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        Quat::IDENTITY,
        scene.nodes[0].base_translation,
    )];

    state.step(&scene, &mut poses, &globals, 0.000_001);

    let expected_rotation = state.bodies[0].rotation * state.bodies[0].local_rotation.conjugate();
    assert!((poses[0].translation - scene.nodes[0].base_translation).length() < 1e-4);
    assert!((poses[0].rotation - expected_rotation).length() < 1e-4);
}

#[test]
fn collision_mask_skips_blocked_pairs() {
    let scene = single_root_scene(vec![
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
    ]);
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.01);

    assert!((state.bodies[1].position.x - 0.4).abs() < 1e-4);
}

#[test]
fn shape_support_radius_differs_from_max_element_fallback() {
    let box_size = Vec3::new(0.1, 2.0, 0.1);
    let capsule_size = Vec3::new(0.1, 2.0, 0.0);

    let box_fallback = helpers::shape_bounding_radius(PmxRigidShape::Box, box_size);
    let box_support = helpers::shape_support_radius(
        &RigidBodyRuntime {
            bone_index: None,
            calc_method: PmxRigidCalcMethod::Dynamic,
            group: 0,
            un_collision_group_flag: 0,
            shape: PmxRigidShape::Box,
            size: box_size,
            local_translation: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            radius: box_fallback,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            inverse_mass: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            repulsion: 0.0,
            friction: 0.0,
        },
        Vec3::X,
    );
    assert!(box_fallback > box_support);

    let capsule_fallback = helpers::shape_bounding_radius(PmxRigidShape::Capsule, capsule_size);
    let capsule_support = helpers::shape_support_radius(
        &RigidBodyRuntime {
            bone_index: None,
            calc_method: PmxRigidCalcMethod::Dynamic,
            group: 0,
            un_collision_group_flag: 0,
            shape: PmxRigidShape::Capsule,
            size: capsule_size,
            local_translation: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            radius: capsule_fallback,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            inverse_mass: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            repulsion: 0.0,
            friction: 0.0,
        },
        Vec3::X,
    );
    assert!(capsule_fallback > capsule_support);
    assert!(
        (helpers::shape_support_radius(
            &RigidBodyRuntime {
                bone_index: None,
                calc_method: PmxRigidCalcMethod::Dynamic,
                group: 0,
                un_collision_group_flag: 0,
                shape: PmxRigidShape::Sphere,
                size: Vec3::splat(0.2),
                local_translation: Vec3::ZERO,
                local_rotation: Quat::IDENTITY,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                radius: 0.2,
                linear_velocity: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                inverse_mass: 1.0,
                linear_damping: 0.0,
                angular_damping: 0.0,
                repulsion: 0.0,
                friction: 0.0,
            },
            Vec3::Z,
        ) - 0.2)
            .abs()
            < 1e-5
    );
}

#[test]
fn shape_aware_box_collision_preserves_narrow_axis_gap() {
    let scene = single_root_scene(vec![
        PmxRigidBodyCpu {
            name: "box".to_owned(),
            name_en: "box".to_owned(),
            bone_index: -1,
            group: 0,
            un_collision_group_flag: 0,
            form: PmxRigidShape::Box,
            size: Vec3::new(0.1, 2.0, 0.1),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            mass: 0.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PmxRigidCalcMethod::Static,
        },
        PmxRigidBodyCpu {
            name: "sphere".to_owned(),
            name_en: "sphere".to_owned(),
            bone_index: -1,
            group: 1,
            un_collision_group_flag: 0,
            form: PmxRigidShape::Sphere,
            size: Vec3::splat(0.1),
            position: Vec3::new(0.25, 0.0, 0.0),
            rotation: Vec3::ZERO,
            mass: 1.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PmxRigidCalcMethod::Dynamic,
        },
    ]);
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.001);

    assert!((state.bodies[1].position.x - 0.25).abs() < 1e-4);
}

#[test]
fn shape_aware_capsule_collision_preserves_narrow_axis_gap() {
    let scene = single_root_scene(vec![
        PmxRigidBodyCpu {
            name: "capsule".to_owned(),
            name_en: "capsule".to_owned(),
            bone_index: -1,
            group: 0,
            un_collision_group_flag: 0,
            form: PmxRigidShape::Capsule,
            size: Vec3::new(0.1, 2.0, 0.0),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            mass: 0.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PmxRigidCalcMethod::Static,
        },
        PmxRigidBodyCpu {
            name: "sphere".to_owned(),
            name_en: "sphere".to_owned(),
            bone_index: -1,
            group: 1,
            un_collision_group_flag: 0,
            form: PmxRigidShape::Sphere,
            size: Vec3::splat(0.1),
            position: Vec3::new(0.25, 0.0, 0.0),
            rotation: Vec3::ZERO,
            mass: 1.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PmxRigidCalcMethod::Dynamic,
        },
    ]);
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.001);

    assert!((state.bodies[1].position.x - 0.25).abs() < 1e-4);
}

#[test]
fn six_dof_joint_clamps_translation_in_joint_frame() {
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
        pmx_physics_meta: Some(PmxPhysicsMeta {
            rigid_bodies: vec![
                PmxRigidBodyCpu {
                    name: "a".to_owned(),
                    name_en: "a".to_owned(),
                    bone_index: -1,
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
                    calc_method: PmxRigidCalcMethod::Dynamic,
                },
                PmxRigidBodyCpu {
                    name: "b".to_owned(),
                    name_en: "b".to_owned(),
                    bone_index: -1,
                    group: 0,
                    un_collision_group_flag: 0,
                    form: PmxRigidShape::Sphere,
                    size: Vec3::splat(0.1),
                    position: Vec3::new(1.0, 0.0, 0.0),
                    rotation: Vec3::ZERO,
                    mass: 1.0,
                    move_resist: 0.0,
                    rotation_resist: 0.0,
                    repulsion: 0.0,
                    friction: 0.0,
                    calc_method: PmxRigidCalcMethod::Dynamic,
                },
            ],
            joints: vec![PmxJointCpu {
                name: "joint".to_owned(),
                name_en: "joint".to_owned(),
                kind: PmxJointKind::SixDof {
                    a_rigid_index: 0,
                    b_rigid_index: 1,
                    position: Vec3::ZERO,
                    rotation: Vec3::ZERO,
                    move_limit_down: Vec3::new(-0.1, -10.0, -10.0),
                    move_limit_up: Vec3::new(0.1, 10.0, 10.0),
                    rotation_limit_down: Vec3::new(-10.0, -10.0, -10.0),
                    rotation_limit_up: Vec3::new(10.0, 10.0, 10.0),
                },
            }],
        }),
        ..SceneCpu::default()
    };
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.05);

    let rel = state.bodies[1].position - state.bodies[0].position;
    assert!(rel.x <= 0.12);
}

#[test]
fn hinge_joint_clamps_rotation_in_joint_frame() {
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
        pmx_physics_meta: Some(PmxPhysicsMeta {
            rigid_bodies: vec![
                PmxRigidBodyCpu {
                    name: "a".to_owned(),
                    name_en: "a".to_owned(),
                    bone_index: -1,
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
                    calc_method: PmxRigidCalcMethod::Dynamic,
                },
                PmxRigidBodyCpu {
                    name: "b".to_owned(),
                    name_en: "b".to_owned(),
                    bone_index: -1,
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
                    calc_method: PmxRigidCalcMethod::Dynamic,
                },
            ],
            joints: vec![PmxJointCpu {
                name: "hinge".to_owned(),
                name_en: "hinge".to_owned(),
                kind: PmxJointKind::Hinge {
                    a_rigid_index: 0,
                    b_rigid_index: 1,
                    low: -0.1,
                    high: 0.1,
                    softness: 0.0,
                    bias_factor: 0.0,
                    relaxation_factor: 0.0,
                    enable_motor: false,
                    target_velocity: 0.0,
                    max_motor_impulse: 0.0,
                },
            }],
        }),
        ..SceneCpu::default()
    };
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    state.bodies[1].rotation = Quat::from_euler(EulerRot::YXZ, 0.0, 0.8, 0.0);
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.05);

    let rel =
        (state.bodies[0].rotation.conjugate() * state.bodies[1].rotation).to_euler(EulerRot::YXZ);
    assert!(rel.1.abs() <= 0.15);
}

#[test]
fn friction_damps_dynamic_velocity() {
    let scene = single_root_scene(vec![PmxRigidBodyCpu {
        name: "rb".to_owned(),
        name_en: "rb".to_owned(),
        bone_index: -1,
        group: 0,
        un_collision_group_flag: 0,
        form: PmxRigidShape::Sphere,
        size: Vec3::splat(0.2),
        position: Vec3::new(0.0, 1.0, 0.0),
        rotation: Vec3::ZERO,
        mass: 1.0,
        move_resist: 0.0,
        rotation_resist: 0.0,
        repulsion: 0.0,
        friction: 1.0,
        calc_method: PmxRigidCalcMethod::Dynamic,
    }]);
    let mut state =
        PmxPhysicsState::from_scene(&scene, RuntimePmxSettings::default()).expect("physics state");
    state.bodies[0].linear_velocity = Vec3::new(6.0, 0.0, 0.0);
    let mut poses = vec![NodePose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    let globals = vec![Mat4::IDENTITY];

    state.step(&scene, &mut poses, &globals, 0.016);

    assert!(state.bodies[0].linear_velocity.x < 6.0);
}

#[test]
fn warmup_and_reset_keep_state_finite() {
    let scene = single_root_scene(vec![PmxRigidBodyCpu {
        name: "rb".to_owned(),
        name_en: "rb".to_owned(),
        bone_index: -1,
        group: 0,
        un_collision_group_flag: 0,
        form: PmxRigidShape::Sphere,
        size: Vec3::splat(0.1),
        position: Vec3::new(0.0, 2.0, 0.0),
        rotation: Vec3::ZERO,
        mass: 1.0,
        move_resist: 0.1,
        rotation_resist: 0.1,
        repulsion: 0.0,
        friction: 0.2,
        calc_method: PmxRigidCalcMethod::Dynamic,
    }]);
    let settings = RuntimePmxSettings {
        warmup_steps: 12,
        ..RuntimePmxSettings::default()
    };
    let mut state = PmxPhysicsState::from_scene(&scene, settings).expect("physics state");

    state.warmup(&scene);
    assert!(state.bodies[0].position.is_finite());
    assert!(state.bodies[0].rotation.is_finite());

    state.bodies[0].position = Vec3::new(99.0, -99.0, 12.0);
    state.reset(&scene);

    assert!((state.bodies[0].position - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-4);
    assert!(state.bodies[0].linear_velocity.length_squared() <= f32::EPSILON);
}
