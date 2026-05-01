use serde_json::json;

use crate::scene::SceneCpu;
use crate::shared::pmx_log;

#[derive(Default)]
pub(super) struct PmxMorphStats {
    pub vertex: usize,
    pub uv: usize,
    pub bone: usize,
    pub material: usize,
    pub group_flip_impulse: usize,
    pub empty: usize,
}

pub(super) fn log_pmx_parity_report(scene: &SceneCpu, morph_stats: &PmxMorphStats) {
    let Some(rig) = scene.pmx_rig_meta.as_ref() else {
        return;
    };
    let Some(physics) = scene.pmx_physics_meta.as_ref() else {
        return;
    };

    let mut joint_links = vec![0usize; physics.rigid_bodies.len()];
    for joint in &physics.joints {
        let (a_rigid_index, b_rigid_index) = match &joint.kind {
            crate::engine::pmx_rig::PmxJointKind::Spring6Dof {
                a_rigid_index,
                b_rigid_index,
                ..
            }
            | crate::engine::pmx_rig::PmxJointKind::SixDof {
                a_rigid_index,
                b_rigid_index,
                ..
            }
            | crate::engine::pmx_rig::PmxJointKind::P2P {
                a_rigid_index,
                b_rigid_index,
                ..
            }
            | crate::engine::pmx_rig::PmxJointKind::ConeTwist {
                a_rigid_index,
                b_rigid_index,
                ..
            }
            | crate::engine::pmx_rig::PmxJointKind::Slider {
                a_rigid_index,
                b_rigid_index,
                ..
            }
            | crate::engine::pmx_rig::PmxJointKind::Hinge {
                a_rigid_index,
                b_rigid_index,
                ..
            } => (*a_rigid_index, *b_rigid_index),
        };
        for rigid_index in [a_rigid_index, b_rigid_index] {
            if rigid_index >= 0
                && let Some(count) = joint_links.get_mut(rigid_index as usize)
            {
                *count += 1;
            }
        }
    }

    let summary = json!({
        "bones": rig.bones.len(),
        "ik_chains": rig.ik_chains.len(),
        "rigid_bodies": physics.rigid_bodies.len(),
        "joints": physics.joints.len(),
        "append_bones": rig.count_bones_with_append(),
        "fixed_axis_bones": rig.count_bones_with_fixed_axis(),
        "local_axis_bones": rig.count_bones_with_local_axis(),
        "external_parent_bones": rig.count_bones_with_external_parent(),
        "grant_bones": rig.count_bones_with_grant(),
        "local_grant_bones": rig.count_bones_with_local_grant(),
        "grant_cycle_bones": rig.grant_cycle_bones.len(),
        "unsupported": {
            "bone_morphs_loaded": morph_stats.bone,
            "bone_morphs_applied": false,
            "group_flip_impulse_loaded": morph_stats.group_flip_impulse,
            "group_flip_impulse_applied": false,
            "external_parent_loaded": rig.count_bones_with_external_parent(),
            "external_parent_applied": false,
            "grant_loaded": rig.count_bones_with_grant(),
            "grant_applied": true,
            "grant_mode": "approximate",
            "local_grant_loaded": rig.count_bones_with_local_grant(),
            "local_grant_applied": true,
            "local_grant_mode": "approximate_local_space",
        }
    });
    pmx_log::info(format!("PMX parity summary: {summary}"));

    for (rigid_index, rigid) in physics.rigid_bodies.iter().enumerate() {
        let record = json!({
            "index": rigid_index,
            "name": rigid.name,
            "bone_index": rigid.bone_index,
            "calc_method": pmx_calc_method_name(rigid.calc_method),
            "shape": pmx_shape_name(rigid.form),
            "mass": rigid.mass,
            "move_resist": rigid.move_resist,
            "rotation_resist": rigid.rotation_resist,
            "repulsion": rigid.repulsion,
            "friction": rigid.friction,
            "joint_links": joint_links.get(rigid_index).copied().unwrap_or(0),
        });
        pmx_log::info(format!("PMX rigid parity: {record}"));
    }
}

fn pmx_calc_method_name(method: crate::engine::pmx_rig::PmxRigidCalcMethod) -> &'static str {
    match method {
        crate::engine::pmx_rig::PmxRigidCalcMethod::Static => "static",
        crate::engine::pmx_rig::PmxRigidCalcMethod::Dynamic => "dynamic",
        crate::engine::pmx_rig::PmxRigidCalcMethod::DynamicWithBonePosition => {
            "dynamic_with_bone_position"
        }
    }
}

fn pmx_shape_name(shape: crate::engine::pmx_rig::PmxRigidShape) -> &'static str {
    match shape {
        crate::engine::pmx_rig::PmxRigidShape::Sphere => "sphere",
        crate::engine::pmx_rig::PmxRigidShape::Box => "box",
        crate::engine::pmx_rig::PmxRigidShape::Capsule => "capsule",
    }
}
