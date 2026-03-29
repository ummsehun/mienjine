use glam::Mat4;

use crate::engine::pmx_rig::{compute_bone_position, solve_ik_chain_ccd};
use crate::runtime::state::PmxPhysicsState;
use crate::{animation::ChannelTarget, scene::NodePose};
use crate::{
    animation::{
        compute_global_matrices_in_place, compute_skin_matrices_in_place, reset_poses_from_nodes,
    },
    scene::SceneCpu,
};

pub struct FramePipeline {
    poses: Vec<NodePose>,
    node_morph_weights: Vec<Vec<f32>>,
    instance_morph_weights: Vec<Vec<f32>>,
    material_morph_weights: Vec<f32>,
    globals: Vec<Mat4>,
    globals_visited: Vec<bool>,
    skin_matrices: Vec<Vec<Mat4>>,
    text_buffer: String,
}

impl FramePipeline {
    pub fn new(scene: &SceneCpu) -> Self {
        Self {
            poses: Vec::with_capacity(scene.nodes.len()),
            node_morph_weights: Vec::with_capacity(scene.nodes.len()),
            instance_morph_weights: Vec::with_capacity(scene.mesh_instances.len()),
            material_morph_weights: vec![0.0; scene.material_morphs.len()],
            globals: Vec::with_capacity(scene.nodes.len()),
            globals_visited: Vec::with_capacity(scene.nodes.len()),
            skin_matrices: Vec::with_capacity(scene.skins.len()),
            text_buffer: String::new(),
        }
    }

    pub(crate) fn prepare_frame(
        &mut self,
        scene: &SceneCpu,
        elapsed_seconds: f32,
        anim_index: Option<usize>,
        mut physics_state: Option<&mut PmxPhysicsState>,
        physics_dt: f32,
    ) {
        reset_poses_from_nodes(&scene.nodes, &mut self.poses);
        seed_node_morph_weights(scene, &mut self.node_morph_weights);
        self.material_morph_weights.fill(0.0);
        let mut primary_normalized_time = None;
        if let Some(index) = anim_index {
            if let Some(clip) = scene.animations.get(index) {
                clip.sample_into_with_morph(
                    elapsed_seconds,
                    &mut self.poses,
                    &mut self.node_morph_weights,
                    &mut self.material_morph_weights,
                );
                primary_normalized_time =
                    Some(normalized_clip_time(elapsed_seconds, clip.duration));
            }
        }
        for (index, clip) in scene.animations.iter().enumerate() {
            if Some(index) == anim_index || !is_morph_only_clip(clip) {
                continue;
            }
            let sample_time = match primary_normalized_time {
                Some(primary_t) if clip.duration > f32::EPSILON => primary_t * clip.duration,
                _ => elapsed_seconds,
            };
            clip.sample_into_with_morph(
                sample_time,
                &mut self.poses,
                &mut self.node_morph_weights,
                &mut self.material_morph_weights,
            );
        }

        // PMX IK solve: adjust joint rotations after animation sampling
        if let Some(rig_meta) = &scene.pmx_rig_meta {
            for chain in &rig_meta.ik_chains {
                let target_pos =
                    compute_bone_position(chain.controller_bone_index, &scene.nodes, &self.poses);
                solve_ik_chain_ccd(chain, &scene.nodes, &mut self.poses, target_pos);
            }
        }

        compute_global_matrices_in_place(
            &scene.nodes,
            &self.poses,
            &mut self.globals,
            &mut self.globals_visited,
        );
        if let Some(physics) = physics_state.as_deref_mut() {
            physics.step(scene, &mut self.poses, &self.globals, physics_dt);
            compute_global_matrices_in_place(
                &scene.nodes,
                &self.poses,
                &mut self.globals,
                &mut self.globals_visited,
            );
        }
        compute_skin_matrices_in_place(scene, &self.globals, &mut self.skin_matrices);
        resolve_instance_morph_weights(
            scene,
            &self.node_morph_weights,
            &mut self.instance_morph_weights,
        );
    }

    pub fn globals(&self) -> &[Mat4] {
        &self.globals
    }

    pub fn skin_matrices(&self) -> &[Vec<Mat4>] {
        &self.skin_matrices
    }

    pub fn morph_weights_by_instance(&self) -> &[Vec<f32>] {
        &self.instance_morph_weights
    }

    pub fn material_morph_weights(&self) -> &[f32] {
        &self.material_morph_weights
    }

    pub fn text_buffer_mut(&mut self) -> &mut String {
        &mut self.text_buffer
    }
}

fn is_morph_only_clip(clip: &crate::animation::AnimationClip) -> bool {
    !clip.channels.is_empty()
        && clip
            .channels
            .iter()
            .all(|channel| channel.target == ChannelTarget::MorphWeights)
}

fn normalized_clip_time(elapsed_seconds: f32, duration: f32) -> f32 {
    if duration <= f32::EPSILON {
        return 0.0;
    }
    elapsed_seconds.rem_euclid(duration) / duration
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{AnimationChannel, AnimationClip, ChannelValues, Interpolation};
    use crate::scene::{MeshCpu, MeshInstance, MeshLayer, MorphTargetCpu, Node, SceneCpu};
    use glam::{Quat, Vec3};

    #[test]
    fn normalized_clip_time_wraps() {
        let t = normalized_clip_time(3.5, 2.0);
        assert!((t - 0.75).abs() < 1e-6);
    }

    #[test]
    fn morph_only_clip_detection() {
        let clip = AnimationClip {
            name: Some("facial".to_owned()),
            channels: vec![AnimationChannel {
                node_index: 0,
                target: ChannelTarget::MorphWeights,
                interpolation: Interpolation::Linear,
                inputs: vec![0.0, 1.0],
                outputs: ChannelValues::MorphWeights {
                    values: vec![0.0, 1.0],
                    weights_per_key: 1,
                },
            }],
            duration: 1.0,
            looping: true,
        };
        assert!(is_morph_only_clip(&clip));
    }

    #[test]
    fn prepare_frame_applies_secondary_morph_clip_with_primary_timeline() {
        let node = Node {
            name: Some("root".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };
        let mesh = MeshCpu {
            positions: vec![Vec3::ZERO],
            normals: vec![Vec3::Y],
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 0, 0]],
            joints4: None,
            weights4: None,
            sdef_vertices: None,
            morph_targets: vec![MorphTargetCpu {
                name: Some("smile".to_owned()),
                position_deltas: vec![Vec3::new(0.0, 1.0, 0.0)],
                normal_deltas: vec![Vec3::ZERO],
            }],
        };
        let primary = AnimationClip {
            name: Some("bone".to_owned()),
            channels: vec![AnimationChannel {
                node_index: 0,
                target: ChannelTarget::Translation,
                interpolation: Interpolation::Linear,
                inputs: vec![0.0, 2.0],
                outputs: ChannelValues::Vec3(vec![Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0)]),
            }],
            duration: 2.0,
            looping: true,
        };
        let facial = AnimationClip {
            name: Some("facial".to_owned()),
            channels: vec![AnimationChannel {
                node_index: 0,
                target: ChannelTarget::MorphWeights,
                interpolation: Interpolation::Linear,
                inputs: vec![0.0, 1.0],
                outputs: ChannelValues::MorphWeights {
                    values: vec![0.0, 1.0],
                    weights_per_key: 1,
                },
            }],
            duration: 1.0,
            looping: true,
        };
        let scene = SceneCpu {
            meshes: vec![mesh],
            materials: Vec::new(),
            textures: Vec::new(),
            skins: Vec::new(),
            nodes: vec![node],
            mesh_instances: vec![MeshInstance {
                mesh_index: 0,
                node_index: 0,
                skin_index: None,
                default_morph_weights: vec![0.0],
                layer: MeshLayer::Subject,
            }],
            animations: vec![primary, facial],
            root_center_node: Some(0),
            pmx_rig_meta: None,
            pmx_physics_meta: None,
            material_morphs: Vec::new(),
        };

        let mut pipeline = FramePipeline::new(&scene);
        pipeline.prepare_frame(&scene, 1.0, Some(0), None, 0.0);
        let applied = pipeline.morph_weights_by_instance()[0][0];
        assert!((applied - 0.5).abs() < 1e-5);
    }

    #[test]
    fn prepare_frame_applies_pmx_physics_before_skinning() {
        let scene = SceneCpu {
            meshes: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            skins: Vec::new(),
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            mesh_instances: Vec::new(),
            animations: Vec::new(),
            root_center_node: Some(0),
            pmx_rig_meta: None,
            pmx_physics_meta: Some(crate::engine::pmx_rig::PmxPhysicsMeta {
                rigid_bodies: vec![crate::engine::pmx_rig::PmxRigidBodyCpu {
                    name: "rb".to_owned(),
                    name_en: "rb".to_owned(),
                    bone_index: 0,
                    group: 0,
                    un_collision_group_flag: 0,
                    form: crate::engine::pmx_rig::PmxRigidShape::Sphere,
                    size: Vec3::splat(0.1),
                    position: Vec3::new(0.0, 1.0, 0.0),
                    rotation: Vec3::ZERO,
                    mass: 1.0,
                    move_resist: 0.0,
                    rotation_resist: 0.0,
                    repulsion: 0.0,
                    friction: 0.0,
                    calc_method: crate::engine::pmx_rig::PmxRigidCalcMethod::Dynamic,
                }],
                joints: Vec::new(),
            }),
            material_morphs: Vec::new(),
        };

        let mut pipeline = FramePipeline::new(&scene);
        let mut physics = PmxPhysicsState::from_scene(&scene).expect("physics state");
        pipeline.prepare_frame(&scene, 0.0, None, Some(&mut physics), 0.2);

        let root_y = pipeline.globals()[0].transform_point3(Vec3::ZERO).y;
        assert!(root_y < 1.0);
    }

    #[test]
    fn prepare_frame_applies_ik_using_controller_target() {
        let scene = SceneCpu {
            meshes: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            skins: Vec::new(),
            nodes: vec![
                Node {
                    name: Some("controller".to_owned()),
                    parent: None,
                    children: Vec::new(),
                    base_translation: Vec3::new(0.0, 1.0, 0.0),
                    base_rotation: Quat::IDENTITY,
                    base_scale: Vec3::ONE,
                },
                Node {
                    name: Some("joint".to_owned()),
                    parent: None,
                    children: vec![2],
                    base_translation: Vec3::ZERO,
                    base_rotation: Quat::IDENTITY,
                    base_scale: Vec3::ONE,
                },
                Node {
                    name: Some("effector".to_owned()),
                    parent: Some(1),
                    children: Vec::new(),
                    base_translation: Vec3::new(1.0, 0.0, 0.0),
                    base_rotation: Quat::IDENTITY,
                    base_scale: Vec3::ONE,
                },
            ],
            mesh_instances: Vec::new(),
            animations: Vec::new(),
            root_center_node: Some(0),
            pmx_rig_meta: Some(crate::engine::pmx_rig::PmxRigMeta {
                ik_chains: vec![crate::engine::pmx_rig::IKChain {
                    controller_bone_index: 0,
                    target_bone_index: 2,
                    chain_root_bone_index: 1,
                    iterations: 8,
                    limit_angle: 1.0,
                    links: vec![crate::engine::pmx_rig::IKLink {
                        bone_index: 1,
                        angle_limits: None,
                    }],
                }],
            }),
            pmx_physics_meta: None,
            material_morphs: Vec::new(),
        };

        let mut pipeline = FramePipeline::new(&scene);
        pipeline.prepare_frame(&scene, 0.0, None, None, 0.0);

        let effector_world = pipeline.globals()[2].transform_point3(Vec3::ZERO);
        assert!(effector_world.y > 0.5);
    }
}

fn seed_node_morph_weights(scene: &SceneCpu, node_morph_weights: &mut Vec<Vec<f32>>) {
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

fn resolve_instance_morph_weights(
    scene: &SceneCpu,
    node_morph_weights: &[Vec<f32>],
    instance_morph_weights: &mut Vec<Vec<f32>>,
) {
    instance_morph_weights.resize_with(scene.mesh_instances.len(), Vec::new);
    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        let dst = &mut instance_morph_weights[instance_index];
        dst.clear();
        if let Some(node_weights) = node_morph_weights.get(instance.node_index) {
            if !node_weights.is_empty() {
                dst.extend_from_slice(node_weights);
                continue;
            }
        }
        if !instance.default_morph_weights.is_empty() {
            dst.extend_from_slice(&instance.default_morph_weights);
        }
    }
}
