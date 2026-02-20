use glam::Mat4;

use crate::{
    animation::{
        compute_global_matrices_in_place, compute_skin_matrices_in_place, reset_poses_from_nodes,
    },
    scene::{NodePose, SceneCpu},
};

pub struct FramePipeline {
    poses: Vec<NodePose>,
    globals: Vec<Mat4>,
    globals_visited: Vec<bool>,
    skin_matrices: Vec<Vec<Mat4>>,
    text_buffer: String,
}

impl FramePipeline {
    pub fn new(scene: &SceneCpu) -> Self {
        Self {
            poses: Vec::with_capacity(scene.nodes.len()),
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
        if let Some(index) = anim_index {
            if let Some(clip) = scene.animations.get(index) {
                clip.sample_into(elapsed_seconds, &mut self.poses);
            }
        }
        compute_global_matrices_in_place(
            &scene.nodes,
            &self.poses,
            &mut self.globals,
            &mut self.globals_visited,
        );
        compute_skin_matrices_in_place(scene, &self.globals, &mut self.skin_matrices);
    }

    pub fn globals(&self) -> &[Mat4] {
        &self.globals
    }

    pub fn skin_matrices(&self) -> &[Vec<Mat4>] {
        &self.skin_matrices
    }

    pub fn text_buffer_mut(&mut self) -> &mut String {
        &mut self.text_buffer
    }
}
