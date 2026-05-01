use crate::animation::ChannelTarget;
use crate::engine::pmx_rig::{
    apply_append_bone_transforms, apply_pmx_bone_axis_constraints, compute_bone_position,
    solve_ik_chain_ccd,
};
use crate::scene::{NodePose, SceneCpu};

pub(crate) fn is_morph_only_clip(clip: &crate::animation::AnimationClip) -> bool {
    !clip.channels.is_empty()
        && clip
            .channels
            .iter()
            .all(|channel| channel.target == ChannelTarget::MorphWeights)
}

pub(crate) fn normalized_clip_time(elapsed_seconds: f32, duration: f32) -> f32 {
    if duration <= f32::EPSILON {
        return 0.0;
    }
    elapsed_seconds.rem_euclid(duration) / duration
}

fn solve_pmx_ik_chains(scene: &SceneCpu, poses: &mut [NodePose], physics_active: bool) {
    let Some(rig_meta) = &scene.pmx_rig_meta else {
        return;
    };

    for chain in &rig_meta.ik_chains {
        if physics_active && ik_chain_conflicts_with_physics(scene, chain) {
            continue;
        }
        let target_pos = compute_bone_position(chain.controller_bone_index, &scene.nodes, poses);
        solve_ik_chain_ccd(chain, &scene.nodes, poses, target_pos);
    }
}

pub(crate) fn apply_pmx_pose_stack(scene: &SceneCpu, poses: &mut [NodePose], physics_active: bool) {
    let Some(rig_meta) = &scene.pmx_rig_meta else {
        return;
    };

    apply_append_bone_transforms(rig_meta, poses);
    solve_pmx_ik_chains(scene, poses, physics_active);
    apply_pmx_bone_axis_constraints(rig_meta, poses);
}

fn ik_chain_conflicts_with_physics(
    scene: &SceneCpu,
    chain: &crate::engine::pmx_rig::IKChain,
) -> bool {
    let Some(physics_meta) = scene.pmx_physics_meta.as_ref() else {
        return false;
    };

    chain.links.iter().any(|link| {
        physics_meta.rigid_bodies.iter().any(|rigid| {
            rigid.bone_index >= 0
                && rigid.bone_index as usize == link.bone_index
                && !matches!(
                    rigid.calc_method,
                    crate::engine::pmx_rig::PmxRigidCalcMethod::Static
                )
        })
    })
}

pub(crate) fn seed_node_morph_weights(scene: &SceneCpu, node_morph_weights: &mut Vec<Vec<f32>>) {
    node_morph_weights.resize_with(scene.nodes.len(), Vec::new);
    for weights in node_morph_weights.iter_mut() {
        weights.clear();
    }
    for instance in &scene.mesh_instances {
        let Some(node_weights) = node_morph_weights.get_mut(instance.node_index) else {
            continue;
        };
        if node_weights.len() < instance.default_morph_weights.len() {
            node_weights.resize(instance.default_morph_weights.len(), 0.0);
        }
        for (i, value) in instance.default_morph_weights.iter().enumerate() {
            node_weights[i] = *value;
        }
    }
}

pub(crate) fn resolve_instance_morph_weights(
    scene: &SceneCpu,
    node_morph_weights: &[Vec<f32>],
    instance_morph_weights: &mut Vec<Vec<f32>>,
) {
    instance_morph_weights.resize_with(scene.mesh_instances.len(), Vec::new);
    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        let dst = &mut instance_morph_weights[instance_index];
        dst.clear();
        if let Some(node_weights) = node_morph_weights.get(instance.node_index)
            && !node_weights.is_empty()
        {
            dst.extend_from_slice(node_weights);
            continue;
        }
        if !instance.default_morph_weights.is_empty() {
            dst.extend_from_slice(&instance.default_morph_weights);
        }
    }
}
