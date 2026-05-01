use glam::{Quat, Vec3};
use gltf::animation::util::ReadOutputs;

use crate::animation::{AnimationChannel, AnimationClip, ChannelTarget, ChannelValues};

use super::texture_utils::{infer_morph_weights_per_key, map_interpolation};

pub(super) fn load_gltf_animations(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    node_morph_target_counts: &[usize],
) -> Vec<AnimationClip> {
    document
        .animations()
        .filter_map(|animation| {
            let mut channels = Vec::new();
            let mut duration = 0.0f32;
            for channel in animation.channels() {
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
                let Some(inputs) = reader.read_inputs() else {
                    continue;
                };
                let inputs = inputs.collect::<Vec<_>>();
                if let Some(last) = inputs.last() {
                    duration = duration.max(*last);
                }
                let interpolation = map_interpolation(channel.sampler().interpolation());
                let node_index = channel.target().node().index();
                let target = match channel.target().property() {
                    gltf::animation::Property::Translation => ChannelTarget::Translation,
                    gltf::animation::Property::Rotation => ChannelTarget::Rotation,
                    gltf::animation::Property::Scale => ChannelTarget::Scale,
                    gltf::animation::Property::MorphTargetWeights => ChannelTarget::MorphWeights,
                };
                let outputs = reader.read_outputs()?;
                let outputs = match outputs {
                    ReadOutputs::Translations(values) => {
                        ChannelValues::Vec3(values.map(Vec3::from_array).collect())
                    }
                    ReadOutputs::Rotations(values) => {
                        let quats = values
                            .into_f32()
                            .map(|q| Quat::from_xyzw(q[0], q[1], q[2], q[3]))
                            .collect();
                        ChannelValues::Quat(quats)
                    }
                    ReadOutputs::Scales(values) => {
                        ChannelValues::Vec3(values.map(Vec3::from_array).collect())
                    }
                    ReadOutputs::MorphTargetWeights(values) => {
                        let raw_values = values.into_f32().collect::<Vec<f32>>();
                        let weights_per_key = infer_morph_weights_per_key(
                            node_morph_target_counts
                                .get(node_index)
                                .copied()
                                .unwrap_or_default(),
                            raw_values.len(),
                            inputs.len(),
                            interpolation,
                        );
                        if weights_per_key == 0 {
                            continue;
                        }
                        ChannelValues::MorphWeights {
                            values: raw_values,
                            weights_per_key,
                        }
                    }
                };
                channels.push(AnimationChannel {
                    node_index,
                    target,
                    interpolation,
                    inputs,
                    outputs,
                });
            }
            if channels.is_empty() {
                None
            } else {
                Some(AnimationClip {
                    name: animation.name().map(ToOwned::to_owned),
                    channels,
                    duration,
                    looping: true,
                })
            }
        })
        .collect::<Vec<_>>()
}
