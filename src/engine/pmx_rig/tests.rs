#[cfg(test)]
mod tests {
    use crate::engine::pmx_rig::bone::apply_append_bone_transforms;
    use crate::engine::pmx_rig::bone::apply_pmx_bone_axis_constraints;
    use crate::engine::pmx_rig::ik::{compute_bone_position, rotation_between};
    use crate::engine::pmx_rig::types::{
        IKChain, IKLink, PmxBoneMeta, PmxGrantTransform, PmxRigMeta,
    };
    use glam::{Quat, Vec3};

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
                name_en: None,
                parent: None,
                children: vec![1],
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
            crate::scene::Node {
                name: Some("child".to_owned()),
                name_en: None,
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

    #[test]
    fn test_apply_append_bone_transforms_blends_parent_pose() {
        let meta = PmxRigMeta {
            bones: vec![
                PmxBoneMeta {
                    grant_transform: None,
                    name: "root".to_owned(),
                    name_en: "root".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: -1,
                    deform_depth: 0,
                    boneflag: 0,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: -1,
                    append_weight: 0.0,
                    fixed_axis: Vec3::ZERO,
                    local_axis_x: Vec3::ZERO,
                    local_axis_z: Vec3::ZERO,
                    key_value: 0,
                    ik_target_index: -1,
                    ik_iter_count: 0,
                    ik_limit: 0.0,
                },
                PmxBoneMeta {
                    grant_transform: Some(PmxGrantTransform {
                        parent_index: 0,
                        weight: 0.5,
                        is_local: false,
                        affects_rotation: true,
                        affects_translation: true,
                    }),
                    name: "child".to_owned(),
                    name_en: "child".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: 0,
                    deform_depth: 1,
                    boneflag: 0x0100 | 0x0200,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: 0,
                    append_weight: 0.5,
                    fixed_axis: Vec3::ZERO,
                    local_axis_x: Vec3::ZERO,
                    local_axis_z: Vec3::ZERO,
                    key_value: 0,
                    ik_target_index: -1,
                    ik_iter_count: 0,
                    ik_limit: 0.0,
                },
            ],
            ik_chains: Vec::new(),
            grant_evaluation_order: vec![1],
            grant_cycle_bones: Vec::new(),
        };
        let mut poses = vec![
            crate::scene::NodePose {
                translation: Vec3::new(2.0, 0.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                scale: Vec3::ONE,
            },
            crate::scene::NodePose {
                translation: Vec3::new(1.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ];

        apply_append_bone_transforms(&meta, &mut poses);

        assert!((poses[1].translation - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5);
        let rotated = poses[1].rotation * Vec3::X;
        assert!((rotated - Vec3::new(0.70710677, 0.70710677, 0.0)).length() < 1e-4);
    }

    #[test]
    fn test_apply_pmx_bone_axis_constraints_projects_fixed_axis_twist() {
        let meta = PmxRigMeta {
            bones: vec![PmxBoneMeta {
                grant_transform: None,
                name: "joint".to_owned(),
                name_en: "joint".to_owned(),
                position: Vec3::ZERO,
                parent_index: -1,
                deform_depth: 0,
                boneflag: 0x0400,
                offset: Vec3::ZERO,
                child: -1,
                append_bone_index: -1,
                append_weight: 0.0,
                fixed_axis: Vec3::Y,
                local_axis_x: Vec3::ZERO,
                local_axis_z: Vec3::ZERO,
                key_value: 0,
                ik_target_index: -1,
                ik_iter_count: 0,
                ik_limit: 0.0,
            }],
            ik_chains: Vec::new(),
            grant_evaluation_order: Vec::new(),
            grant_cycle_bones: Vec::new(),
        };
        let mut poses = vec![crate::scene::NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::from_euler(glam::EulerRot::XYZ, 0.45, 0.35, -0.2),
            scale: Vec3::ONE,
        }];

        apply_pmx_bone_axis_constraints(&meta, &mut poses);

        let axis_after = poses[0].rotation * Vec3::Y;
        assert!((axis_after - Vec3::Y).length() < 1e-4);
    }

    #[test]
    fn test_apply_pmx_bone_axis_constraints_keeps_local_axis_metadata_passive() {
        let meta = PmxRigMeta {
            bones: vec![PmxBoneMeta {
                grant_transform: None,
                name: "joint".to_owned(),
                name_en: "joint".to_owned(),
                position: Vec3::ZERO,
                parent_index: -1,
                deform_depth: 0,
                boneflag: 0x0800,
                offset: Vec3::ZERO,
                child: -1,
                append_bone_index: -1,
                append_weight: 0.0,
                fixed_axis: Vec3::ZERO,
                local_axis_x: Vec3::Y,
                local_axis_z: Vec3::Z,
                key_value: 0,
                ik_target_index: -1,
                ik_iter_count: 0,
                ik_limit: 0.0,
            }],
            ik_chains: Vec::new(),
            grant_evaluation_order: Vec::new(),
            grant_cycle_bones: Vec::new(),
        };
        let mut poses = vec![crate::scene::NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::from_rotation_x(0.5),
            scale: Vec3::ONE,
        }];

        let before = poses[0].rotation;
        apply_pmx_bone_axis_constraints(&meta, &mut poses);

        assert!(poses[0].rotation.dot(before).abs() > 0.999_99);
        assert!((poses[0].rotation.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_grant_evaluation_order_applies_parent_before_child() {
        let mut meta = PmxRigMeta {
            bones: vec![
                PmxBoneMeta {
                    grant_transform: None,
                    name: "root".to_owned(),
                    name_en: "root".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: -1,
                    deform_depth: 0,
                    boneflag: 0,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: -1,
                    append_weight: 0.0,
                    fixed_axis: Vec3::ZERO,
                    local_axis_x: Vec3::ZERO,
                    local_axis_z: Vec3::ZERO,
                    key_value: 0,
                    ik_target_index: -1,
                    ik_iter_count: 0,
                    ik_limit: 0.0,
                },
                PmxBoneMeta {
                    grant_transform: Some(PmxGrantTransform {
                        parent_index: 0,
                        weight: 0.5,
                        is_local: false,
                        affects_rotation: false,
                        affects_translation: true,
                    }),
                    name: "mid".to_owned(),
                    name_en: "mid".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: 0,
                    deform_depth: 1,
                    boneflag: 0x0200,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: 0,
                    append_weight: 0.5,
                    fixed_axis: Vec3::ZERO,
                    local_axis_x: Vec3::ZERO,
                    local_axis_z: Vec3::ZERO,
                    key_value: 0,
                    ik_target_index: -1,
                    ik_iter_count: 0,
                    ik_limit: 0.0,
                },
                PmxBoneMeta {
                    grant_transform: Some(PmxGrantTransform {
                        parent_index: 1,
                        weight: 1.0,
                        is_local: false,
                        affects_rotation: false,
                        affects_translation: true,
                    }),
                    name: "leaf".to_owned(),
                    name_en: "leaf".to_owned(),
                    position: Vec3::ZERO,
                    parent_index: 1,
                    deform_depth: 2,
                    boneflag: 0x0200,
                    offset: Vec3::ZERO,
                    child: -1,
                    append_bone_index: 1,
                    append_weight: 1.0,
                    fixed_axis: Vec3::ZERO,
                    local_axis_x: Vec3::ZERO,
                    local_axis_z: Vec3::ZERO,
                    key_value: 0,
                    ik_target_index: -1,
                    ik_iter_count: 0,
                    ik_limit: 0.0,
                },
            ],
            ik_chains: Vec::new(),
            grant_evaluation_order: Vec::new(),
            grant_cycle_bones: Vec::new(),
        };
        meta.rebuild_grant_evaluation_order();
        assert_eq!(meta.grant_evaluation_order, vec![1, 2]);

        let mut poses = vec![
            crate::scene::NodePose {
                translation: Vec3::new(2.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            crate::scene::NodePose {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            crate::scene::NodePose {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ];

        apply_append_bone_transforms(&meta, &mut poses);

        assert!((poses[1].translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5);
        assert!((poses[2].translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5);
    }
}
