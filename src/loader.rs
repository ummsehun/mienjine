use std::path::Path;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use gltf::animation::util::ReadOutputs;
use tobj::LoadOptions;

use crate::animation::{
    AnimationChannel, AnimationClip, ChannelTarget, ChannelValues, Interpolation,
};
use crate::scene::{MeshCpu, MeshInstance, Node, SceneCpu, SkinCpu};

pub fn load_obj(path: &Path) -> Result<SceneCpu> {
    let options = LoadOptions {
        triangulate: true,
        single_index: true,
        ..LoadOptions::default()
    };
    let (models, _) = tobj::load_obj(path, &options)
        .with_context(|| format!("failed to load OBJ: {}", path.display()))?;
    if models.is_empty() {
        bail!("OBJ has no mesh models: {}", path.display());
    }

    let mut scene = SceneCpu::default();
    for model in models {
        let mesh_data = model.mesh;
        if mesh_data.positions.is_empty() {
            continue;
        }
        let positions = mesh_data
            .positions
            .chunks_exact(3)
            .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
            .collect::<Vec<_>>();
        let mut normals = mesh_data
            .normals
            .chunks_exact(3)
            .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
            .collect::<Vec<_>>();
        let indices = mesh_data
            .indices
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect::<Vec<_>>();

        if normals.len() != positions.len() {
            normals = compute_vertex_normals(&positions, &indices);
        }
        let mesh_index = scene.meshes.len();
        scene.meshes.push(MeshCpu {
            positions,
            normals,
            indices,
            joints4: None,
            weights4: None,
        });
        let node_index = scene.nodes.len();
        scene.nodes.push(Node {
            name: Some(model.name),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        });
        scene.mesh_instances.push(MeshInstance {
            mesh_index,
            node_index,
            skin_index: None,
        });
    }
    if scene.meshes.is_empty() {
        bail!("OBJ has no renderable geometry: {}", path.display());
    }
    Ok(scene)
}

pub fn load_gltf(path: &Path) -> Result<SceneCpu> {
    let (document, buffers, _images) = gltf::import(path)
        .with_context(|| format!("failed to import GLB/glTF: {}", path.display()))?;

    let mut nodes = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            Node {
                name: node.name().map(ToOwned::to_owned),
                parent: None,
                children: node.children().map(|child| child.index()).collect(),
                base_translation: Vec3::from_array(translation),
                base_rotation: Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
                base_scale: Vec3::from_array(scale),
            }
        })
        .collect::<Vec<_>>();

    for parent_idx in 0..nodes.len() {
        let children = nodes[parent_idx].children.clone();
        for child_idx in children {
            if let Some(child) = nodes.get_mut(child_idx) {
                child.parent = Some(parent_idx);
            }
        }
    }

    let mut skins = Vec::new();
    for skin in document.skins() {
        let joints = skin.joints().map(|joint| joint.index()).collect::<Vec<_>>();
        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()].0));
        let inverse_bind_mats = if let Some(iter) = reader.read_inverse_bind_matrices() {
            iter.map(|m| Mat4::from_cols_array_2d(&m))
                .collect::<Vec<_>>()
        } else {
            vec![Mat4::IDENTITY; joints.len()]
        };
        skins.push(SkinCpu {
            joints,
            inverse_bind_mats,
        });
    }

    let mut meshes = Vec::new();
    let mut mesh_instances = Vec::new();
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let node_index = node.index();
        let skin_index = node.skin().map(|skin| skin.index());

        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
            let positions = reader
                .read_positions()
                .map(|iter| iter.map(Vec3::from_array).collect::<Vec<_>>())
                .context("triangle primitive missing POSITION attribute")?;

            let mut normals = reader
                .read_normals()
                .map(|iter| iter.map(Vec3::from_array).collect::<Vec<_>>())
                .unwrap_or_default();

            let flat_indices = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect::<Vec<_>>())
                .unwrap_or_else(|| (0..(positions.len() as u32)).collect::<Vec<_>>());
            let indices = flat_indices
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect::<Vec<_>>();

            if normals.len() != positions.len() {
                normals = compute_vertex_normals(&positions, &indices);
            }

            let joints4 = reader
                .read_joints(0)
                .map(|iter| iter.into_u16().collect::<Vec<[u16; 4]>>());
            let weights4 = reader
                .read_weights(0)
                .map(|iter| iter.into_f32().collect::<Vec<[f32; 4]>>());
            let (joints4, weights4) = match (joints4, weights4) {
                (Some(joints), Some(weights))
                    if joints.len() == positions.len() && weights.len() == positions.len() =>
                {
                    (Some(joints), Some(weights))
                }
                _ => (None, None),
            };

            let mesh_index = meshes.len();
            meshes.push(MeshCpu {
                positions,
                normals,
                indices,
                joints4,
                weights4,
            });
            mesh_instances.push(MeshInstance {
                mesh_index,
                node_index,
                skin_index,
            });
        }
    }

    let animations = document
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
                    gltf::animation::Property::MorphTargetWeights => continue,
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
                    ReadOutputs::MorphTargetWeights(_) => continue,
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
        .collect::<Vec<_>>();

    if meshes.is_empty() {
        bail!(
            "GLB/glTF has no renderable triangle primitives: {}",
            path.display()
        );
    }

    Ok(SceneCpu {
        meshes,
        skins,
        nodes,
        mesh_instances,
        animations,
    })
}

fn map_interpolation(value: gltf::animation::Interpolation) -> Interpolation {
    match value {
        gltf::animation::Interpolation::Linear => Interpolation::Linear,
        gltf::animation::Interpolation::Step => Interpolation::Step,
        gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
    }
}

fn compute_vertex_normals(positions: &[Vec3], indices: &[[u32; 3]]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for index in indices {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(index[0] as usize),
            positions.get(index[1] as usize),
            positions.get(index[2] as usize),
        ) else {
            continue;
        };
        let n = (*b - *a).cross(*c - *a);
        normals[index[0] as usize] += n;
        normals[index[1] as usize] += n;
        normals[index[2] as usize] += n;
    }
    normals
        .into_iter()
        .map(|n| {
            if n.length_squared() <= f32::EPSILON {
                Vec3::new(0.0, 1.0, 0.0)
            } else {
                n.normalize()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

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
        push_f32s(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]); // positions
        push_f32s(&mut buf, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]); // normals
        push_u16s(&mut buf, &[0, 1, 2]); // indices
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
        push_f32s(&mut buf, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]); // positions, 36
        push_f32s(&mut buf, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]); // normals, 72
        push_u8s(
            &mut buf,
            &[
                0, 0, 0, 0, // joints
                0, 0, 0, 0, 0, 0, 0, 0,
            ],
        ); // 84
        push_f32s(
            &mut buf,
            &[
                1.0, 0.0, 0.0, 0.0, // weights
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
            ],
        ); // 132
        push_u16s(&mut buf, &[0, 1, 2]); // 138
        push_padding(&mut buf, 4); // 140
        push_f32s(
            &mut buf,
            &[
                1.0, 0.0, 0.0, 0.0, // inverse bind matrix (identity)
                0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        ); // 204
        push_f32s(&mut buf, &[0.0, 1.0]); // times, 212
        push_f32s(&mut buf, &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0]); // translations, 236
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

    fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn push_u16s(out: &mut Vec<u8>, values: &[u16]) {
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn push_u8s(out: &mut Vec<u8>, values: &[u8]) {
        out.extend_from_slice(values);
    }

    fn push_padding(out: &mut Vec<u8>, align: usize) {
        let rem = out.len() % align;
        if rem == 0 {
            return;
        }
        let pad = align - rem;
        out.extend(std::iter::repeat(0_u8).take(pad));
    }
}
