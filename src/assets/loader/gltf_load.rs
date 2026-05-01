use std::path::Path;

use crate::scene::{
    MaterialAlphaMode, MaterialCpu, MeshCpu, MeshInstance, MeshLayer, MorphTargetCpu, Node,
    SceneCpu, SkinCpu, TextureColorSpace, TextureFilterMode, TextureWrapMode,
};
use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec2, Vec3};

use super::gltf_animation::load_gltf_animations;
use super::gltf_support::{unsupported_used_extensions, validate_supported_required_extensions};
use super::texture_utils::{
    classify_texture_color_spaces, convert_image_to_texture, convert_texture_transform,
    map_mag_filter, map_min_filter, map_wrap_mode, resolve_default_morph_weights,
};
use super::util::{compute_vertex_normals, find_root_center_node};

pub(super) fn load_gltf_impl(path: &Path) -> Result<SceneCpu> {
    let (document, buffers, images) = gltf::import(path)
        .with_context(|| format!("failed to import GLB/glTF: {}", path.display()))?;
    validate_supported_required_extensions(&document, path)?;
    let unsupported_used_extensions = unsupported_used_extensions(&document);
    if !unsupported_used_extensions.is_empty() {
        eprintln!(
            "warning: GLB/glTF uses unsupported optional extension(s) [{}]; related features may be ignored.",
            unsupported_used_extensions.join(", ")
        );
    }

    let mut nodes = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            Node {
                name: node.name().map(ToOwned::to_owned),
                name_en: None,
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

    let mut unsupported_texture_formats = 0usize;
    let mut textures = images
        .iter()
        .map(|image| match convert_image_to_texture(image) {
            Some(texture) => texture,
            None => {
                unsupported_texture_formats = unsupported_texture_formats.saturating_add(1);
                super::texture_utils::fallback_white_texture()
            }
        })
        .collect::<Vec<_>>();
    if unsupported_texture_formats > 0 {
        eprintln!(
            "warning: {} texture(s) used unsupported image format and were replaced with white fallback.",
            unsupported_texture_formats
        );
    }
    let texture_color_spaces = classify_texture_color_spaces(&document, textures.len());
    for (index, texture) in textures.iter_mut().enumerate() {
        texture.color_space = texture_color_spaces
            .get(index)
            .copied()
            .unwrap_or(TextureColorSpace::Srgb);
    }
    let materials = document
        .materials()
        .map(|material| {
            let pbr = material.pbr_metallic_roughness();
            let (
                base_color_texture,
                base_color_tex_coord,
                base_color_uv_transform,
                base_color_wrap_s,
                base_color_wrap_t,
                base_color_min_filter,
                base_color_mag_filter,
            ) = if let Some(texture_info) = pbr.base_color_texture() {
                let texture_index = texture_info.texture().source().index();
                let sampler = texture_info.texture().sampler();
                (
                    (texture_index < textures.len()).then_some(texture_index),
                    texture_info.tex_coord(),
                    convert_texture_transform(&texture_info),
                    map_wrap_mode(sampler.wrap_s()),
                    map_wrap_mode(sampler.wrap_t()),
                    map_min_filter(sampler.min_filter()),
                    map_mag_filter(sampler.mag_filter()),
                )
            } else {
                (
                    None,
                    0,
                    None,
                    TextureWrapMode::Repeat,
                    TextureWrapMode::Repeat,
                    TextureFilterMode::Linear,
                    TextureFilterMode::Linear,
                )
            };
            let base_color_factor = pbr.base_color_factor();
            let emissive_factor = material.emissive_factor();
            let alpha_mode = match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => MaterialAlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => MaterialAlphaMode::Mask,
                gltf::material::AlphaMode::Blend => MaterialAlphaMode::Blend,
            };
            MaterialCpu {
                base_color_factor,
                base_color_texture,
                base_color_tex_coord,
                base_color_uv_transform,
                base_color_wrap_s,
                base_color_wrap_t,
                base_color_min_filter,
                base_color_mag_filter,
                sphere_texture: None,
                toon_source: None,
                emissive_factor,
                alpha_mode,
                alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5).clamp(0.0, 1.0),
                double_sided: material.double_sided(),
            }
        })
        .collect::<Vec<_>>();

    let mut meshes = Vec::new();
    let mut mesh_instances = Vec::new();
    let mut node_morph_target_counts = vec![0usize; nodes.len()];
    let mut warned_missing_uv1 = false;
    let mut warned_unsupported_tex_coord = false;
    let mut skipped_non_triangle_primitives = 0usize;
    let mut dropped_uv0_primitives = 0usize;
    let mut dropped_uv1_primitives = 0usize;
    let mut dropped_color_primitives = 0usize;
    let mut padded_morph_position_targets = 0usize;
    let mut padded_morph_normal_targets = 0usize;
    let mut warned_renderer_ignored_normal_texture = false;
    let mut warned_renderer_ignored_emissive_texture = false;
    let mut warned_renderer_ignored_occlusion_texture = false;
    let mut warned_renderer_ignored_metallic_roughness_texture = false;
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let node_index = node.index();
        let skin_index = node.skin().map(|skin| skin.index());

        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                skipped_non_triangle_primitives = skipped_non_triangle_primitives.saturating_add(1);
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
            let uv0 = reader.read_tex_coords(0).map(|iter| {
                iter.into_f32()
                    .map(|uv| Vec2::new(uv[0], uv[1]))
                    .collect::<Vec<_>>()
            });
            let uv1 = reader.read_tex_coords(1).map(|iter| {
                iter.into_f32()
                    .map(|uv| Vec2::new(uv[0], uv[1]))
                    .collect::<Vec<_>>()
            });
            let colors_rgba = reader.read_colors(0).map(|iter| {
                iter.into_rgba_f32()
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect::<Vec<_>>()
            });

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

            let morph_targets = reader
                .read_morph_targets()
                .map(|(target_positions, target_normals, _)| {
                    let mut position_deltas = target_positions
                        .map(|iter| iter.map(Vec3::from_array).collect::<Vec<_>>())
                        .unwrap_or_default();
                    if position_deltas.len() != positions.len() {
                        padded_morph_position_targets =
                            padded_morph_position_targets.saturating_add(1);
                        position_deltas.resize(positions.len(), Vec3::ZERO);
                    }
                    let mut normal_deltas = target_normals
                        .map(|iter| iter.map(Vec3::from_array).collect::<Vec<_>>())
                        .unwrap_or_default();
                    if normal_deltas.len() != positions.len() {
                        padded_morph_normal_targets = padded_morph_normal_targets.saturating_add(1);
                        normal_deltas.resize(positions.len(), Vec3::ZERO);
                    }
                    MorphTargetCpu {
                        name: None,
                        position_deltas,
                        normal_deltas,
                        uv0_deltas: None,
                        uv1_deltas: None,
                    }
                })
                .collect::<Vec<_>>();
            if let Some(slot) = node_morph_target_counts.get_mut(node_index) {
                *slot = (*slot).max(morph_targets.len());
            }
            let default_morph_weights =
                resolve_default_morph_weights(node.weights(), mesh.weights(), morph_targets.len());

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
            let uv0 = match uv0 {
                Some(values) if values.len() == positions.len() => Some(values),
                Some(_) => {
                    dropped_uv0_primitives = dropped_uv0_primitives.saturating_add(1);
                    None
                }
                None => None,
            };
            let uv1 = match uv1 {
                Some(values) if values.len() == positions.len() => Some(values),
                Some(_) => {
                    dropped_uv1_primitives = dropped_uv1_primitives.saturating_add(1);
                    None
                }
                None => None,
            };
            let colors_rgba = match colors_rgba {
                Some(values) if values.len() == positions.len() => Some(values),
                Some(_) => {
                    dropped_color_primitives = dropped_color_primitives.saturating_add(1);
                    None
                }
                None => None,
            };
            let material_index = primitive
                .material()
                .index()
                .filter(|index| *index < materials.len());
            if let Some(material) = material_index.and_then(|index| materials.get(index)) {
                if !warned_renderer_ignored_normal_texture
                    && primitive.material().normal_texture().is_some()
                {
                    warned_renderer_ignored_normal_texture = true;
                    eprintln!(
                        "warning: normal textures are loaded but ignored by the terminal renderer."
                    );
                }
                if !warned_renderer_ignored_emissive_texture
                    && primitive.material().emissive_texture().is_some()
                {
                    warned_renderer_ignored_emissive_texture = true;
                    eprintln!(
                        "warning: emissive textures are loaded but ignored by the terminal renderer."
                    );
                }
                if !warned_renderer_ignored_occlusion_texture
                    && primitive.material().occlusion_texture().is_some()
                {
                    warned_renderer_ignored_occlusion_texture = true;
                    eprintln!(
                        "warning: occlusion textures are loaded but ignored by the terminal renderer."
                    );
                }
                if !warned_renderer_ignored_metallic_roughness_texture
                    && primitive
                        .material()
                        .pbr_metallic_roughness()
                        .metallic_roughness_texture()
                        .is_some()
                {
                    warned_renderer_ignored_metallic_roughness_texture = true;
                    eprintln!(
                        "warning: metallic-roughness textures are loaded but ignored by the terminal renderer."
                    );
                }
                let requested_tex_coord = material
                    .base_color_uv_transform
                    .and_then(|transform| transform.tex_coord_override)
                    .unwrap_or(material.base_color_tex_coord);
                if requested_tex_coord == 1 && uv1.is_none() && !warned_missing_uv1 {
                    warned_missing_uv1 = true;
                    eprintln!(
                        "warning: TEXCOORD_1 requested by material but missing on primitive. falling back to TEXCOORD_0."
                    );
                } else if requested_tex_coord > 1 && !warned_unsupported_tex_coord {
                    warned_unsupported_tex_coord = true;
                    eprintln!(
                        "warning: TEXCOORD_{} is unsupported for baseColorTexture. using TEXCOORD_0 fallback.",
                        requested_tex_coord
                    );
                }
            }

            let mesh_index = meshes.len();
            meshes.push(MeshCpu {
                positions,
                normals,
                uv0,
                uv1,
                colors_rgba,
                material_index,
                indices,
                joints4,
                weights4,
                sdef_vertices: None,
                morph_targets,
            });
            mesh_instances.push(MeshInstance {
                mesh_index,
                node_index,
                skin_index,
                default_morph_weights,
                layer: MeshLayer::Subject,
            });
        }
    }

    if skipped_non_triangle_primitives > 0 {
        eprintln!(
            "warning: {} primitive(s) were skipped because only triangle primitives are rendered.",
            skipped_non_triangle_primitives
        );
    }
    if dropped_uv0_primitives > 0 {
        eprintln!(
            "warning: {} primitive(s) had invalid TEXCOORD_0 length and were loaded without UV0.",
            dropped_uv0_primitives
        );
    }
    if dropped_uv1_primitives > 0 {
        eprintln!(
            "warning: {} primitive(s) had invalid TEXCOORD_1 length and were loaded without UV1.",
            dropped_uv1_primitives
        );
    }
    if dropped_color_primitives > 0 {
        eprintln!(
            "warning: {} primitive(s) had invalid COLOR_0 length and vertex colors were ignored.",
            dropped_color_primitives
        );
    }
    if padded_morph_position_targets > 0 || padded_morph_normal_targets > 0 {
        eprintln!(
            "warning: morph targets were padded for {} position target(s) and {} normal target(s) whose lengths did not match the base mesh.",
            padded_morph_position_targets, padded_morph_normal_targets
        );
    }

    let animations = load_gltf_animations(&document, &buffers, &node_morph_target_counts);

    if meshes.is_empty() {
        bail!(
            "GLB/glTF has no renderable triangle primitives: {}",
            path.display()
        );
    }

    let root_center_node = find_root_center_node(&nodes);
    Ok(SceneCpu {
        meshes,
        materials,
        textures,
        skins,
        nodes,
        mesh_instances,
        animations,
        root_center_node,
        pmx_rig_meta: None,
        pmx_physics_meta: None,
        material_morphs: Vec::new(),
    })
}
