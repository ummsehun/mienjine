use glam::{Mat4, Quat, Vec3};

use crate::engine::animation::{compute_global_matrices_in_place, reset_poses_from_nodes};
use crate::engine::pipeline::PhysicsStepper;
use crate::engine::pmx_rig::{PmxRigidCalcMethod, PmxRigidShape};
use crate::scene::{NodePose, SceneCpu};

use super::RuntimePmxSettings;

mod helpers;
mod init;
mod profile;
mod step;

#[cfg(test)]
mod tests;

pub(crate) use profile::derive_pmx_profile;
use profile::PmxDerivedProfile;

#[derive(Debug, Clone)]
pub(crate) struct PmxPhysicsState {
    settings: RuntimePmxSettings,
    bodies: Vec<RigidBodyRuntime>,
    joints: Vec<JointRuntime>,
    profile: PmxDerivedProfile,
}

#[derive(Debug, Clone)]
struct RigidBodyRuntime {
    bone_index: Option<usize>,
    calc_method: PmxRigidCalcMethod,
    group: u8,
    un_collision_group_flag: u16,
    shape: PmxRigidShape,
    size: Vec3,
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
    friction: f32,
}

#[derive(Debug, Clone)]
struct JointRuntime {
    kind: JointRuntimeKind,
    a_rigid_index: usize,
    b_rigid_index: usize,
    joint_position: Vec3,
    joint_rotation: Quat,
    rest_offset: Vec3,
    strength: f32,
    spring_const_move: Option<Vec3>,
    spring_const_rotation: Option<Vec3>,
    a_anchor_local: Option<Vec3>,
    b_anchor_local: Option<Vec3>,
    move_limit_down: Option<Vec3>,
    move_limit_up: Option<Vec3>,
    rotation_limit_down: Option<Vec3>,
    rotation_limit_up: Option<Vec3>,
    lower_linear_limit: Option<f32>,
    upper_linear_limit: Option<f32>,
    lower_angle_limit: Option<f32>,
    upper_angle_limit: Option<f32>,
    cone_swing_span1: Option<f32>,
    cone_swing_span2: Option<f32>,
    cone_twist_span: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JointRuntimeKind {
    Spring6Dof,
    SixDof,
    P2P,
    ConeTwist,
    Slider,
    Hinge,
}

impl PmxPhysicsState {
    pub(crate) fn from_scene(scene: &SceneCpu, settings: RuntimePmxSettings) -> Option<Self> {
        init::from_scene(scene, settings)
    }

    pub(crate) fn step(
        &mut self,
        scene: &SceneCpu,
        poses: &mut [NodePose],
        pre_physics_globals: &[Mat4],
        dt: f32,
    ) {
        step::step(self, scene, poses, pre_physics_globals, dt)
    }

    pub(crate) fn warmup(&mut self, scene: &SceneCpu) {
        if self.settings.warmup_steps == 0 {
            return;
        }

        let mut poses = Vec::new();
        reset_poses_from_nodes(&scene.nodes, &mut poses);
        let mut globals = Vec::new();
        let mut visited = Vec::new();
        compute_global_matrices_in_place(&scene.nodes, &poses, &mut globals, &mut visited);
        for _ in 0..self.settings.warmup_steps {
            step::step(self, scene, &mut poses, &globals, self.settings.unit_step);
        }
    }

    pub(crate) fn reset(&mut self, scene: &SceneCpu) {
        if let Some(state) = init::from_scene(scene, self.settings) {
            *self = state;
        }
    }
}

impl PhysicsStepper for PmxPhysicsState {
    fn step_physics(
        &mut self,
        scene: &SceneCpu,
        poses: &mut [NodePose],
        pre_physics_globals: &[Mat4],
        dt: f32,
    ) {
        self.step(scene, poses, pre_physics_globals, dt);
    }
}
