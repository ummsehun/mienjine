use glam::{Quat, Vec3};

use crate::engine::pmx_rig::types::PmxRigMeta;

/// Apply PMX append rotation/translation in a best-effort way.
///
/// This preserves the effect of "additional parent" bones within the current
/// simplified `NodePose` representation. It does not attempt to reproduce the
/// full PMX local-space inheritance matrix model.
pub fn apply_append_bone_transforms(meta: &PmxRigMeta, poses: &mut [crate::scene::NodePose]) {
    let mut resolved_poses = poses.to_vec();
    let grant_order = if meta.grant_evaluation_order.is_empty() {
        (0..meta.bones.len()).collect::<Vec<_>>()
    } else {
        meta.grant_evaluation_order.clone()
    };

    for bone_index in grant_order {
        let Some(bone) = meta.bones.get(bone_index) else {
            continue;
        };
        let Some(grant) = bone.grant_transform.as_ref() else {
            continue;
        };
        let source_index = grant.parent_index;
        if bone_index >= poses.len()
            || source_index >= resolved_poses.len()
            || source_index == bone_index
        {
            continue;
        }
        let weight = grant.weight.clamp(0.0, 1.0);
        if weight <= f32::EPSILON {
            continue;
        }

        let source_pose = resolved_poses[source_index];
        if let Some(target_pose) = poses.get_mut(bone_index) {
            if grant.affects_translation {
                let translated = if grant.is_local {
                    target_pose.rotation * (source_pose.translation * weight)
                } else {
                    source_pose.translation * weight
                };
                target_pose.translation += translated;
            }
            if grant.affects_rotation {
                let append_rotation = Quat::IDENTITY.slerp(source_pose.rotation, weight);
                target_pose.rotation = if grant.is_local {
                    (target_pose.rotation * append_rotation).normalize()
                } else {
                    (append_rotation * target_pose.rotation).normalize()
                };
            }
            resolved_poses[bone_index] = *target_pose;
        }
    }
}

/// Apply PMX fixed-axis and local-axis rotation hints in a best-effort way.
///
/// This does not recreate Blender's full bone constraint system. It only
/// reduces the most visible axis drift by re-basing bones with local axes and
/// constraining fixed-axis bones to twist around their declared axis.
pub fn apply_pmx_bone_axis_constraints(meta: &PmxRigMeta, poses: &mut [crate::scene::NodePose]) {
    for (bone_index, bone) in meta.bones.iter().enumerate() {
        if bone_index >= poses.len() {
            continue;
        }

        let mut rotation = poses[bone_index].rotation;

        if bone.uses_fixed_axis() {
            let fixed_axis = bone.fixed_axis.normalize_or_zero();

            if fixed_axis.length_squared() > f32::EPSILON {
                rotation = twist_only(rotation, fixed_axis);
            }
        }

        poses[bone_index].rotation = rotation.normalize();
    }
}

fn twist_only(rotation: Quat, axis: Vec3) -> Quat {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() <= f32::EPSILON {
        return rotation;
    }

    let vector = Vec3::new(rotation.x, rotation.y, rotation.z);
    let projected = axis * vector.dot(axis);
    let twist = Quat::from_xyzw(projected.x, projected.y, projected.z, rotation.w);
    if twist.length_squared() <= f32::EPSILON {
        rotation
    } else {
        twist.normalize()
    }
}
