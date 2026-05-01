use glam::Mat4;

use crate::engine::skeleton::{
    compute_global_matrices_in_place, compute_skin_matrices_in_place, reset_poses_from_nodes,
};
use crate::scene::{NodePose, SceneCpu};

use super::PhysicsStepper;
use super::helpers::{
    apply_pmx_pose_stack, is_morph_only_clip, normalized_clip_time, resolve_instance_morph_weights,
    seed_node_morph_weights,
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
        physics_state: Option<&mut dyn PhysicsStepper>,
        physics_dt: f32,
    ) {
        reset_poses_from_nodes(&scene.nodes, &mut self.poses);
        seed_node_morph_weights(scene, &mut self.node_morph_weights);
        self.material_morph_weights.fill(0.0);
        let mut primary_normalized_time = None;
        if let Some(index) = anim_index
            && let Some(clip) = scene.animations.get(index)
        {
            clip.sample_into_with_morph(
                elapsed_seconds,
                &mut self.poses,
                &mut self.node_morph_weights,
                &mut self.material_morph_weights,
            );
            primary_normalized_time = Some(normalized_clip_time(elapsed_seconds, clip.duration));
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

        apply_pmx_pose_stack(scene, &mut self.poses, physics_state.is_some());

        compute_global_matrices_in_place(
            &scene.nodes,
            &self.poses,
            &mut self.globals,
            &mut self.globals_visited,
        );
        if let Some(physics) = physics_state {
            physics.step_physics(scene, &mut self.poses, &self.globals, physics_dt);
            apply_pmx_pose_stack(scene, &mut self.poses, true);
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
