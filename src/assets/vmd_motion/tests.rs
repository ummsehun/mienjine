use encoding_rs::SHIFT_JIS;
use glam::{Quat, Vec3};

use crate::animation::ChannelTarget;
use crate::scene::{MeshCpu, MeshInstance, MeshLayer, MorphTargetCpu, Node, SceneCpu};

use super::{VmdBoneFrame, VmdMotion};

#[test]
fn bytes_to_name_decodes_shift_jis() {
    let (encoded, _, _) = SHIFT_JIS.encode("全身");
    let encoded = encoded.into_owned();
    let mut bytes = [0_u8; 15];
    bytes[..encoded.len()].copy_from_slice(&encoded);
    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: Vec::new(),
        morph_frames: Vec::new(),
    };
    assert_eq!(motion.duration_secs(), 0.0);
    assert_eq!(super::parse::bytes_to_name(&bytes), "全身");
}

#[test]
fn to_clip_for_scene_matches_shift_jis_bone_names() {
    let scene = SceneCpu {
        meshes: vec![MeshCpu {
            positions: vec![Vec3::ZERO],
            normals: vec![Vec3::Y],
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 0, 0]],
            joints4: None,
            weights4: None,
            sdef_vertices: None,
            morph_targets: vec![MorphTargetCpu {
                name: Some("smile".to_owned()),
                position_deltas: vec![Vec3::ZERO],
                normal_deltas: vec![Vec3::ZERO],
                uv0_deltas: None,
                uv1_deltas: None,
            }],
        }],
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: Some("全身".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: vec![MeshInstance {
            mesh_index: 0,
            node_index: 0,
            skin_index: None,
            default_morph_weights: vec![0.0],
            layer: MeshLayer::Subject,
        }],
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    };

    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: vec![VmdBoneFrame {
            bone_name: "全身".to_owned(),
            frame_no: 0,
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: Quat::IDENTITY,
        }],
        morph_frames: Vec::new(),
    };

    let clip = motion.to_clip_for_scene(&scene);
    assert_eq!(clip.channels.len(), 2);
}

#[test]
fn to_clip_for_scene_applies_translation_as_bind_pose_offset() {
    let scene = SceneCpu {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: Some("center".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::new(1.0, 2.0, 3.0),
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: Vec::new(),
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    };

    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: vec![VmdBoneFrame {
            bone_name: "center".to_owned(),
            frame_no: 0,
            translation: Vec3::new(0.25, -0.5, 1.0),
            rotation: Quat::IDENTITY,
        }],
        morph_frames: Vec::new(),
    };

    let clip = motion.to_clip_for_scene(&scene);
    let translation_channel = clip
        .channels
        .iter()
        .find(|channel| channel.target == ChannelTarget::Translation)
        .expect("missing translation channel");

    let crate::animation::ChannelValues::Vec3(values) = &translation_channel.outputs else {
        panic!("translation channel must have Vec3 outputs");
    };

    assert_eq!(values.len(), 1);
    assert_eq!(values[0], Vec3::new(1.25, 1.5, 4.0));
}

#[test]
fn to_clip_for_scene_matches_exact_english_node_names() {
    let scene = SceneCpu {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: None,
            name_en: Some("lower body".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: Vec::new(),
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    };

    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: vec![VmdBoneFrame {
            bone_name: "lower body".to_owned(),
            frame_no: 0,
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: Quat::IDENTITY,
        }],
        morph_frames: Vec::new(),
    };

    let clip = motion.to_clip_for_scene(&scene);
    assert_eq!(clip.channels.len(), 2);
}

#[test]
fn to_clip_for_scene_matches_normalized_node_names_too() {
    let scene = SceneCpu {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: Some("下半身".to_owned()),
            name_en: Some("lower body".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: Vec::new(),
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    };

    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: vec![VmdBoneFrame {
            bone_name: "LowerBody".to_owned(),
            frame_no: 0,
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: Quat::IDENTITY,
        }],
        morph_frames: Vec::new(),
    };

    let clip = motion.to_clip_for_scene(&scene);
    assert_eq!(clip.channels.len(), 2);
}

#[test]
fn to_clip_for_scene_matches_head_aliases() {
    let scene = SceneCpu {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: Some("頭".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: Vec::new(),
        animations: Vec::new(),
        root_center_node: Some(0),
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    };

    let motion = VmdMotion {
        model_name: "test".to_owned(),
        bone_frames: vec![VmdBoneFrame {
            bone_name: "head".to_owned(),
            frame_no: 0,
            translation: Vec3::new(0.0, 0.25, 0.0),
            rotation: Quat::IDENTITY,
        }],
        morph_frames: Vec::new(),
    };

    let clip = motion.to_clip_for_scene(&scene);
    assert_eq!(clip.channels.len(), 2);
}
