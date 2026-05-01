use std::path::Path;

use anyhow::Result;
use glam::{Quat, Vec3};

use crate::animation::AnimationClip;
use crate::scene::SceneCpu;

mod clip;
mod parse;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct VmdMotion {
    pub model_name: String,
    pub bone_frames: Vec<VmdBoneFrame>,
    pub morph_frames: Vec<VmdMorphFrame>,
}

impl VmdMotion {
    pub fn duration_secs(&self) -> f32 {
        clip::duration_secs(self)
    }

    pub fn to_clip_for_scene(&self, scene: &SceneCpu) -> AnimationClip {
        clip::to_clip_for_scene(self, scene)
    }
}

#[derive(Debug, Clone)]
pub struct VmdBoneFrame {
    pub bone_name: String,
    pub frame_no: u32,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Debug, Clone)]
pub struct VmdMorphFrame {
    pub morph_name: String,
    pub frame_no: u32,
    pub weight: f32,
}

pub fn parse_vmd_motion(path: &Path) -> Result<VmdMotion> {
    parse::parse_vmd_motion(path)
}
