use glam::Mat4;

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
            globals: Vec::with_capacity(scene.nodes.len()),
            globals_visited: Vec::with_capacity(scene.nodes.len()),
            skin_matrices: Vec::with_capacity(scene.skins.len()),
            text_buffer: String::new(),
        }
    }

    pub fn prepare_frame(
        &mut self,
        scene: &SceneCpu,
        elapsed_seconds: f32,
        anim_index: Option<usize>,
    ) {
        reset_poses_from_nodes(&scene.nodes, &mut self.poses);
        seed_node_morph_weights(scene, &mut self.node_morph_weights);
        let mut primary_normalized_time = None;
        if let Some(index) = anim_index {
            if let Some(clip) = scene.animations.get(index) {
                clip.sample_into_with_morph(
                    elapsed_seconds,
                    &mut self.poses,
                    &mut self.node_morph_weights,
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
            clip.sample_into_with_morph(sample_time, &mut self.poses, &mut self.node_morph_weights);
        }
        compute_global_matrices_in_place(
            &scene.nodes,
            &self.poses,
            &mut self.globals,
            &mut self.globals_visited,
        );
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
    use crate::scene::{MeshCpu, MeshInstance, MorphTargetCpu, Node, SceneCpu};
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
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 0, 0]],
            joints4: None,
            weights4: None,
            morph_targets: vec![MorphTargetCpu {
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
            }],
            animations: vec![primary, facial],
            root_center_node: Some(0),
        };

        let mut pipeline = FramePipeline::new(&scene);
        pipeline.prepare_frame(&scene, 1.0, Some(0));
        let applied = pipeline.morph_weights_by_instance()[0][0];
        assert!((applied - 0.5).abs() < 1e-5);
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
