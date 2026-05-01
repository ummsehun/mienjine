use std::path::Path;

use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use tobj::LoadOptions;

use crate::scene::{MeshCpu, MeshInstance, MeshLayer, Node, SceneCpu};

use super::util::{compute_vertex_normals, find_root_center_node};

pub(super) fn load_obj_impl(path: &Path) -> Result<SceneCpu> {
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
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices,
            joints4: None,
            weights4: None,
            sdef_vertices: None,
            morph_targets: Vec::new(),
        });
        let node_index = scene.nodes.len();
        scene.nodes.push(Node {
            name: Some(model.name),
            name_en: None,
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
            default_morph_weights: Vec::new(),
            layer: MeshLayer::Subject,
        });
    }
    if scene.meshes.is_empty() {
        bail!("OBJ has no renderable geometry: {}", path.display());
    }
    scene.root_center_node = find_root_center_node(&scene.nodes);
    Ok(scene)
}
