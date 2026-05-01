use std::collections::BTreeMap;

use glam::{Quat, Vec3};

use crate::animation::{
    AnimationChannel, AnimationClip, ChannelTarget, ChannelValues, Interpolation,
};
use crate::scene::{Node, SceneCpu};

use super::{VmdMorphFrame, VmdMotion};

pub(super) fn duration_secs(motion: &VmdMotion) -> f32 {
    let max_frame = motion
        .bone_frames
        .iter()
        .map(|frame| frame.frame_no)
        .chain(motion.morph_frames.iter().map(|frame| frame.frame_no))
        .max()
        .unwrap_or(0);
    max_frame as f32 / 30.0
}

pub(super) fn to_clip_for_scene(motion: &VmdMotion, scene: &SceneCpu) -> AnimationClip {
    let mut by_bone: BTreeMap<String, BoneTrack> = BTreeMap::new();
    for frame in &motion.bone_frames {
        let Some(node_index) = scene
            .nodes
            .iter()
            .position(|node| node_matches_vmd_bone(node, frame.bone_name.as_str()))
        else {
            continue;
        };
        let entry = by_bone
            .entry(frame.bone_name.clone())
            .or_insert_with(|| BoneTrack {
                node_index,
                translations: BTreeMap::new(),
                rotations: BTreeMap::new(),
            });
        entry.node_index = node_index;
        entry.translations.insert(frame.frame_no, frame.translation);
        entry.rotations.insert(frame.frame_no, frame.rotation);
    }

    let mut channels = Vec::new();
    for track in by_bone.into_values() {
        let base_translation = scene
            .nodes
            .get(track.node_index)
            .map(|node| node.base_translation)
            .unwrap_or(Vec3::ZERO);
        let mut translation_inputs = Vec::with_capacity(track.translations.len());
        let mut translation_outputs = Vec::with_capacity(track.translations.len());
        for (frame_no, value) in track.translations {
            translation_inputs.push(frame_no as f32 / 30.0);
            translation_outputs.push(base_translation + value);
        }
        if !translation_inputs.is_empty() {
            channels.push(AnimationChannel {
                node_index: track.node_index,
                target: ChannelTarget::Translation,
                interpolation: Interpolation::Linear,
                inputs: translation_inputs,
                outputs: ChannelValues::Vec3(translation_outputs),
            });
        }

        let mut rotation_inputs = Vec::with_capacity(track.rotations.len());
        let mut rotation_outputs = Vec::with_capacity(track.rotations.len());
        for (frame_no, value) in track.rotations {
            rotation_inputs.push(frame_no as f32 / 30.0);
            rotation_outputs.push(value);
        }
        if !rotation_inputs.is_empty() {
            channels.push(AnimationChannel {
                node_index: track.node_index,
                target: ChannelTarget::Rotation,
                interpolation: Interpolation::Linear,
                inputs: rotation_inputs,
                outputs: ChannelValues::Quat(rotation_outputs),
            });
        }
    }

    if let Some((node_index, morph_index_map)) = scene.mesh_instances.iter().find_map(|instance| {
        let mesh = scene.meshes.get(instance.mesh_index)?;
        if mesh.morph_targets.is_empty() {
            return None;
        }
        let mut map = BTreeMap::new();
        for (index, morph) in mesh.morph_targets.iter().enumerate() {
            if let Some(name) = morph.name.as_deref() {
                map.insert(name.to_owned(), index);
            }
        }
        if map.is_empty() {
            None
        } else {
            Some((instance.node_index, map))
        }
    }) {
        push_morph_weights(
            &motion.morph_frames,
            node_index,
            &morph_index_map,
            ChannelTarget::MorphWeights,
            &mut channels,
        );
    }

    if !scene.material_morphs.is_empty() {
        let mut mat_morph_map: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, morph) in scene.material_morphs.iter().enumerate() {
            mat_morph_map.insert(morph.name.as_str(), index);
        }

        let mut by_mat_morph: BTreeMap<usize, BTreeMap<u32, f32>> = BTreeMap::new();
        for frame in &motion.morph_frames {
            let Some(&morph_index) = mat_morph_map.get(frame.morph_name.as_str()) else {
                continue;
            };
            by_mat_morph
                .entry(morph_index)
                .or_default()
                .insert(frame.frame_no, frame.weight);
        }

        if !by_mat_morph.is_empty() {
            let weights_per_key = mat_morph_map.len();
            let (inputs, outputs) = assemble_morph_channels(&by_mat_morph, weights_per_key);
            if !inputs.is_empty() {
                channels.push(AnimationChannel {
                    node_index: 0,
                    target: ChannelTarget::MaterialMorphWeights,
                    interpolation: Interpolation::Linear,
                    inputs,
                    outputs: ChannelValues::MorphWeights {
                        values: outputs,
                        weights_per_key,
                    },
                });
            }
        }
    }

    AnimationClip {
        name: Some(motion.model_name.clone()),
        channels,
        duration: duration_secs(motion).max(1.0 / 30.0),
        looping: true,
    }
}

fn push_morph_weights(
    frames: &[VmdMorphFrame],
    node_index: usize,
    morph_index_map: &BTreeMap<String, usize>,
    target: ChannelTarget,
    channels: &mut Vec<AnimationChannel>,
) {
    let mut by_morph: BTreeMap<usize, BTreeMap<u32, f32>> = BTreeMap::new();
    for frame in frames {
        let Some(&morph_index) = morph_index_map.get(&frame.morph_name) else {
            continue;
        };
        by_morph
            .entry(morph_index)
            .or_default()
            .insert(frame.frame_no, frame.weight);
    }

    if by_morph.is_empty() {
        return;
    }

    let weights_per_key = morph_index_map.len();
    let (inputs, outputs) = assemble_morph_channels(&by_morph, weights_per_key);
    if !inputs.is_empty() {
        channels.push(AnimationChannel {
            node_index,
            target,
            interpolation: Interpolation::Linear,
            inputs,
            outputs: ChannelValues::MorphWeights {
                values: outputs,
                weights_per_key,
            },
        });
    }
}

fn assemble_morph_channels(
    by_morph: &BTreeMap<usize, BTreeMap<u32, f32>>,
    weights_per_key: usize,
) -> (Vec<f32>, Vec<f32>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut key_frames: Vec<u32> = by_morph
        .values()
        .flat_map(|frames| frames.keys().copied())
        .collect();
    key_frames.sort_unstable();
    key_frames.dedup();
    for frame_no in key_frames {
        inputs.push(frame_no as f32 / 30.0);
        let mut row = vec![0.0; weights_per_key];
        for (&morph_index, frames) in by_morph {
            if let Some(weight) = frames.get(&frame_no) {
                row[morph_index] = *weight;
            }
        }
        outputs.extend(row);
    }
    (inputs, outputs)
}

fn node_matches_vmd_bone(node: &Node, bone_name: &str) -> bool {
    for candidate in [node.name.as_deref(), node.name_en.as_deref()]
        .into_iter()
        .flatten()
    {
        if candidate == bone_name {
            return true;
        }

        let candidate_keys = bone_name_match_keys(candidate);
        let bone_keys = bone_name_match_keys(bone_name);
        if candidate_keys
            .iter()
            .any(|candidate_key| bone_keys.iter().any(|bone_key| bone_key == candidate_key))
        {
            return true;
        }
    }

    false
}

fn bone_name_match_keys(name: &str) -> Vec<String> {
    let normalized = normalize_bone_name(name);
    let mut keys = vec![normalized.clone()];
    for alias in curated_bone_name_aliases(normalized.as_str()) {
        if !keys.iter().any(|existing| existing == alias) {
            keys.push((*alias).to_owned());
        }
    }
    keys
}

fn normalize_bone_name(name: &str) -> String {
    name.trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n' | '\r' | '_' | '-' | '・' | '.'))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn curated_bone_name_aliases(name: &str) -> &'static [&'static str] {
    match name {
        "head" | "頭" => &["head", "頭"],
        "lowerbody" | "下半身" => &["lowerbody", "下半身"],
        "upperbody" | "上半身" => &["upperbody", "上半身"],
        "neck" | "首" => &["neck", "首"],
        "center" | "センター" | "全ての親" | "全身" => {
            &["center", "センター", "全ての親", "全身"]
        }
        _ => &[],
    }
}

#[derive(Debug, Clone)]
struct BoneTrack {
    node_index: usize,
    translations: BTreeMap<u32, Vec3>,
    rotations: BTreeMap<u32, Quat>,
}
