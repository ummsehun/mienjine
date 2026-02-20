use glam::{Mat4, Quat, Vec3};

use crate::scene::{Node, NodePose, SceneCpu};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTarget {
    Translation,
    Rotation,
    Scale,
}

#[derive(Debug, Clone)]
pub enum ChannelValues {
    Vec3(Vec<Vec3>),
    Quat(Vec<Quat>),
}

#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub node_index: usize,
    pub target: ChannelTarget,
    pub interpolation: Interpolation,
    pub inputs: Vec<f32>,
    pub outputs: ChannelValues,
}

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: Option<String>,
    pub channels: Vec<AnimationChannel>,
    pub duration: f32,
    pub looping: bool,
}

impl AnimationClip {
    pub fn sample_into(&self, time: f32, poses: &mut [NodePose]) {
        if self.channels.is_empty() || self.duration <= f32::EPSILON {
            return;
        }

        let sampled_time = if self.looping {
            time.rem_euclid(self.duration.max(f32::EPSILON))
        } else {
            time.clamp(0.0, self.duration)
        };

        for channel in &self.channels {
            if channel.inputs.is_empty() || channel.node_index >= poses.len() {
                continue;
            }
            let (i0, i1, t) = sample_segment(&channel.inputs, sampled_time);
            match (&channel.target, &channel.outputs) {
                (ChannelTarget::Translation, ChannelValues::Vec3(values)) => {
                    let v0 = vec3_at(values, i0, channel.interpolation);
                    let v1 = vec3_at(values, i1, channel.interpolation);
                    let value = interpolate_vec3(v0, v1, channel.interpolation, t);
                    poses[channel.node_index].translation = value;
                }
                (ChannelTarget::Scale, ChannelValues::Vec3(values)) => {
                    let v0 = vec3_at(values, i0, channel.interpolation);
                    let v1 = vec3_at(values, i1, channel.interpolation);
                    let value = interpolate_vec3(v0, v1, channel.interpolation, t);
                    poses[channel.node_index].scale = value;
                }
                (ChannelTarget::Rotation, ChannelValues::Quat(values)) => {
                    let q0 = quat_at(values, i0, channel.interpolation);
                    let q1 = quat_at(values, i1, channel.interpolation);
                    let value = interpolate_quat(q0, q1, channel.interpolation, t);
                    poses[channel.node_index].rotation = value;
                }
                _ => {}
            }
        }
    }
}

fn sample_segment(inputs: &[f32], time: f32) -> (usize, usize, f32) {
    if inputs.len() == 1 || time <= inputs[0] {
        return (0, 0, 0.0);
    }
    let last = inputs.len() - 1;
    if time >= inputs[last] {
        return (last, last, 0.0);
    }
    let upper = inputs.partition_point(|v| *v < time);
    let i1 = upper.min(last);
    let i0 = i1.saturating_sub(1);
    let t0 = inputs[i0];
    let t1 = inputs[i1];
    let alpha = if (t1 - t0).abs() <= f32::EPSILON {
        0.0
    } else {
        (time - t0) / (t1 - t0)
    };
    (i0, i1, alpha.clamp(0.0, 1.0))
}

fn vec3_at(values: &[Vec3], key_index: usize, interpolation: Interpolation) -> Vec3 {
    match interpolation {
        Interpolation::CubicSpline => {
            let index = key_index.saturating_mul(3).saturating_add(1);
            values.get(index).copied().unwrap_or(Vec3::ZERO)
        }
        _ => values.get(key_index).copied().unwrap_or(Vec3::ZERO),
    }
}

fn quat_at(values: &[Quat], key_index: usize, interpolation: Interpolation) -> Quat {
    match interpolation {
        Interpolation::CubicSpline => {
            let index = key_index.saturating_mul(3).saturating_add(1);
            values.get(index).copied().unwrap_or(Quat::IDENTITY)
        }
        _ => values.get(key_index).copied().unwrap_or(Quat::IDENTITY),
    }
}

fn interpolate_vec3(v0: Vec3, v1: Vec3, interpolation: Interpolation, t: f32) -> Vec3 {
    match interpolation {
        Interpolation::Step => v0,
        Interpolation::Linear | Interpolation::CubicSpline => v0.lerp(v1, t),
    }
}

fn interpolate_quat(q0: Quat, q1: Quat, interpolation: Interpolation, t: f32) -> Quat {
    match interpolation {
        Interpolation::Step => q0,
        Interpolation::Linear | Interpolation::CubicSpline => q0.slerp(q1, t),
    }
}

pub fn default_poses(nodes: &[Node]) -> Vec<NodePose> {
    let mut poses = Vec::with_capacity(nodes.len());
    reset_poses_from_nodes(nodes, &mut poses);
    poses
}

pub fn compute_global_matrices(nodes: &[Node], poses: &[NodePose]) -> Vec<Mat4> {
    let mut globals = Vec::with_capacity(nodes.len());
    let mut visited = Vec::with_capacity(nodes.len());
    compute_global_matrices_in_place(nodes, poses, &mut globals, &mut visited);
    globals
}

fn compute_node_global(
    index: usize,
    nodes: &[Node],
    poses: &[NodePose],
    globals: &mut [Mat4],
    visited: &mut [bool],
) -> Mat4 {
    if visited[index] {
        return globals[index];
    }
    let local = poses
        .get(index)
        .copied()
        .unwrap_or(NodePose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        })
        .to_mat4();
    let global = if let Some(parent) = nodes[index].parent {
        compute_node_global(parent, nodes, poses, globals, visited) * local
    } else {
        local
    };
    globals[index] = global;
    visited[index] = true;
    global
}

pub fn compute_skin_matrices(scene: &SceneCpu, global_matrices: &[Mat4]) -> Vec<Vec<Mat4>> {
    let mut skin_matrices = Vec::with_capacity(scene.skins.len());
    compute_skin_matrices_in_place(scene, global_matrices, &mut skin_matrices);
    skin_matrices
}

pub fn reset_poses_from_nodes(nodes: &[Node], poses: &mut Vec<NodePose>) {
    poses.clear();
    poses.extend(nodes.iter().map(NodePose::from));
}

pub fn compute_global_matrices_in_place(
    nodes: &[Node],
    poses: &[NodePose],
    globals: &mut Vec<Mat4>,
    visited: &mut Vec<bool>,
) {
    globals.resize(nodes.len(), Mat4::IDENTITY);
    visited.resize(nodes.len(), false);
    visited.fill(false);
    for index in 0..nodes.len() {
        compute_node_global(
            index,
            nodes,
            poses,
            globals.as_mut_slice(),
            visited.as_mut_slice(),
        );
    }
}

pub fn compute_skin_matrices_in_place(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &mut Vec<Vec<Mat4>>,
) {
    skin_matrices.resize_with(scene.skins.len(), Vec::new);
    for (skin_index, skin) in scene.skins.iter().enumerate() {
        let matrices = &mut skin_matrices[skin_index];
        matrices.resize(skin.joints.len(), Mat4::IDENTITY);
        for (joint_slot, joint_node) in skin.joints.iter().enumerate() {
            let joint_global = global_matrices
                .get(*joint_node)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            let inverse_bind = skin
                .inverse_bind_mats
                .get(joint_slot)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            matrices[joint_slot] = joint_global * inverse_bind;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Node;

    #[test]
    fn animation_linear_loop_sampling() {
        let nodes = vec![Node {
            name: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }];
        let mut poses = default_poses(&nodes);
        let clip = AnimationClip {
            name: Some("move".to_owned()),
            channels: vec![AnimationChannel {
                node_index: 0,
                target: ChannelTarget::Translation,
                interpolation: Interpolation::Linear,
                inputs: vec![0.0, 1.0],
                outputs: ChannelValues::Vec3(vec![Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0)]),
            }],
            duration: 1.0,
            looping: true,
        };
        clip.sample_into(1.25, &mut poses);
        assert!((poses[0].translation.y - 0.5).abs() < 1e-5);
    }

    #[test]
    fn global_matrix_parent_chain() {
        let nodes = vec![
            Node {
                name: None,
                parent: None,
                children: vec![1],
                base_translation: Vec3::new(1.0, 0.0, 0.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
            Node {
                name: None,
                parent: Some(0),
                children: Vec::new(),
                base_translation: Vec3::new(0.0, 2.0, 0.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
        ];
        let poses = default_poses(&nodes);
        let globals = compute_global_matrices(&nodes, &poses);
        let p = globals[1].transform_point3(Vec3::ZERO);
        assert!((p.x - 1.0).abs() < 1e-5);
        assert!((p.y - 2.0).abs() < 1e-5);
    }
}
