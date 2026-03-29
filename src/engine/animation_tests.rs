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

#[test]
fn morph_weights_linear_sampling() {
    let nodes = vec![Node {
        name: Some("morph".to_owned()),
        parent: None,
        children: Vec::new(),
        base_translation: Vec3::ZERO,
        base_rotation: Quat::IDENTITY,
        base_scale: Vec3::ONE,
    }];
    let mut poses = default_poses(&nodes);
    let mut morph_weights = vec![vec![0.0, 0.0]];
    let clip = AnimationClip {
        name: Some("face".to_owned()),
        channels: vec![AnimationChannel {
            node_index: 0,
            target: ChannelTarget::MorphWeights,
            interpolation: Interpolation::Linear,
            inputs: vec![0.0, 1.0],
            outputs: ChannelValues::MorphWeights {
                values: vec![0.0, 0.0, 1.0, 0.5],
                weights_per_key: 2,
            },
        }],
        duration: 1.0,
        looping: true,
    };

    clip.sample_into_with_morph(0.5, &mut poses, &mut morph_weights, &mut []);
    assert!((morph_weights[0][0] - 0.5).abs() < 1e-5);
    assert!((morph_weights[0][1] - 0.25).abs() < 1e-5);
}
