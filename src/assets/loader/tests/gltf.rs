use std::fs;

use tempfile::tempdir;

use crate::animation::{ChannelTarget, ChannelValues};

use super::super::{load_gltf, load_obj};
use super::common::{push_f32s, push_padding, push_u8s, push_u16s};

#[test]
fn load_obj_triangle() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("tri.obj");
    fs::write(
        &path,
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nvn 0 0 1\nvn 0 0 1\nf 1//1 2//2 3//3\n",
    )
    .expect("write obj");
    let scene = load_obj(&path).expect("load obj");
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(scene.total_triangles(), 1);
    assert_eq!(scene.total_vertices(), 3);
}

#[test]
fn load_gltf_static_triangle() {
    let dir = tempdir().expect("tempdir");
    let gltf_path = dir.path().join("static.gltf");
    let bin_path = dir.path().join("buf.bin");
    let mut buf = Vec::new();
    push_f32s(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    push_f32s(&mut buf, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    push_u16s(&mut buf, &[0, 1, 2]);
    fs::write(&bin_path, &buf).expect("write bin");
    fs::write(
        &gltf_path,
        format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "buf.bin", "byteLength": {}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 72, "byteLength": 6, "target": 34963}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}},
    {{"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1}}, "indices": 2}}]}}],
  "nodes": [{{"mesh": 0}}],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
            buf.len()
        ),
    )
    .expect("write gltf");

    let scene = load_gltf(&gltf_path).expect("load gltf");
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(scene.total_triangles(), 1);
    assert_eq!(scene.animations.len(), 0);
}

#[test]
fn load_gltf_skinned_animation() {
    let dir = tempdir().expect("tempdir");
    let gltf_path = dir.path().join("skinned.gltf");
    let bin_path = dir.path().join("buf.bin");

    let mut buf = Vec::new();
    push_f32s(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    push_f32s(&mut buf, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    push_u8s(&mut buf, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    push_f32s(
        &mut buf,
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    );
    push_u16s(&mut buf, &[0, 1, 2]);
    push_padding(&mut buf, 4);
    push_f32s(
        &mut buf,
        &[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    );
    push_f32s(&mut buf, &[0.0, 1.0]);
    push_f32s(&mut buf, &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    fs::write(&bin_path, &buf).expect("write bin");

    let gltf = format!(
        r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "buf.bin", "byteLength": {byte_len}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 72, "byteLength": 12, "target": 34962}},
    {{"buffer": 0, "byteOffset": 84, "byteLength": 48, "target": 34962}},
    {{"buffer": 0, "byteOffset": 132, "byteLength": 6, "target": 34963}},
    {{"buffer": 0, "byteOffset": 140, "byteLength": 64}},
    {{"buffer": 0, "byteOffset": 204, "byteLength": 8}},
    {{"buffer": 0, "byteOffset": 212, "byteLength": 24}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}},
    {{"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5121, "count": 3, "type": "VEC4"}},
    {{"bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC4"}},
    {{"bufferView": 4, "componentType": 5123, "count": 3, "type": "SCALAR"}},
    {{"bufferView": 5, "componentType": 5126, "count": 1, "type": "MAT4"}},
    {{"bufferView": 6, "componentType": 5126, "count": 2, "type": "SCALAR"}},
    {{"bufferView": 7, "componentType": 5126, "count": 2, "type": "VEC3"}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1, "JOINTS_0": 2, "WEIGHTS_0": 3}}, "indices": 4}}]}}],
  "nodes": [
    {{"mesh": 0, "skin": 0, "children": [1]}},
    {{"translation": [0, 0, 0]}}
  ],
  "skins": [
    {{"joints": [1], "inverseBindMatrices": 5}}
  ],
  "animations": [
    {{
      "name": "lift",
      "samplers": [{{"input": 6, "output": 7, "interpolation": "LINEAR"}}],
      "channels": [{{"sampler": 0, "target": {{"node": 1, "path": "translation"}}}}]
    }}
  ],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
        byte_len = buf.len()
    );
    fs::write(&gltf_path, gltf).expect("write gltf");
    let scene = load_gltf(&gltf_path).expect("load gltf");
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(scene.skins.len(), 1);
    assert_eq!(scene.animations.len(), 1);
    assert!(scene.meshes[0].joints4.is_some());
    assert!(scene.meshes[0].weights4.is_some());
}

#[test]
fn load_gltf_morph_target_animation() {
    let dir = tempdir().expect("tempdir");
    let gltf_path = dir.path().join("morph.gltf");
    let bin_path = dir.path().join("buf.bin");

    let mut buf = Vec::new();
    push_f32s(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    push_f32s(&mut buf, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    push_u16s(&mut buf, &[0, 1, 2]);
    push_padding(&mut buf, 4);
    push_f32s(&mut buf, &[0.0, 0.2, 0.0, 0.0, 0.2, 0.0, 0.0, 0.2, 0.0]);
    push_f32s(&mut buf, &[0.0, 1.0]);
    push_f32s(&mut buf, &[0.0, 1.0]);
    fs::write(&bin_path, &buf).expect("write bin");

    let gltf = format!(
        r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "buf.bin", "byteLength": {byte_len}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 72, "byteLength": 6, "target": 34963}},
    {{"buffer": 0, "byteOffset": 80, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 116, "byteLength": 8}},
    {{"buffer": 0, "byteOffset": 124, "byteLength": 8}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}},
    {{"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}},
    {{"bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR"}},
    {{"bufferView": 5, "componentType": 5126, "count": 2, "type": "SCALAR"}}
  ],
  "meshes": [
    {{
      "weights": [0.0],
      "primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1}}, "indices": 2, "targets": [{{"POSITION": 3}}]}}]
    }}
  ],
  "nodes": [{{"mesh": 0}}],
  "animations": [
    {{
      "name": "face",
      "samplers": [{{"input": 4, "output": 5, "interpolation": "LINEAR"}}],
      "channels": [{{"sampler": 0, "target": {{"node": 0, "path": "weights"}}}}]
    }}
  ],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
        byte_len = buf.len()
    );
    fs::write(&gltf_path, gltf).expect("write gltf");

    let scene = load_gltf(&gltf_path).expect("load gltf");
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(scene.meshes[0].morph_targets.len(), 1);
    assert_eq!(scene.mesh_instances.len(), 1);
    assert_eq!(scene.mesh_instances[0].default_morph_weights.len(), 1);
    assert_eq!(scene.animations.len(), 1);
    let channel = &scene.animations[0].channels[0];
    assert_eq!(channel.target, ChannelTarget::MorphWeights);
    match &channel.outputs {
        ChannelValues::MorphWeights {
            values,
            weights_per_key,
        } => {
            assert_eq!(*weights_per_key, 1);
            assert_eq!(values.len(), 2);
        }
        _ => panic!("expected morph channel outputs"),
    }
}
