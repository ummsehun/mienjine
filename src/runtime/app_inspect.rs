use std::collections::BTreeMap;

use anyhow::{Context, Result};

use crate::{
    animation::ChannelTarget,
    loader,
    runtime::cli::InspectArgs,
    runtime::scene_analysis::{compute_scene_framing, scene_stats_world},
    scene::RenderConfig,
};

pub(crate) fn inspect(args: InspectArgs) -> Result<()> {
    let raw = gltf::Gltf::open(&args.glb)
        .with_context(|| format!("failed to parse glTF metadata: {}", args.glb.display()))?;
    let unsupported_required_extensions = loader::unsupported_required_extensions(&raw);
    let unsupported_used_extensions = loader::unsupported_used_extensions(&raw);
    let scene = loader::load_gltf(&args.glb)?;
    let extensions_required = raw
        .extensions_required()
        .map(|name| name.to_owned())
        .collect::<Vec<_>>();
    let extensions_used = raw
        .extensions_used()
        .map(|name| name.to_owned())
        .collect::<Vec<_>>();
    let mut khr_texture_transform_primitives = 0usize;
    let mut texcoord_override_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut texcoord_base_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut non_triangle_primitives = 0usize;
    let mut normal_texture_primitives = 0usize;
    let mut emissive_texture_primitives = 0usize;
    let mut occlusion_texture_primitives = 0usize;
    let mut metallic_roughness_texture_primitives = 0usize;
    let mut double_sided_materials = 0usize;
    for mesh in raw.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                non_triangle_primitives = non_triangle_primitives.saturating_add(1);
            }
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            if let Some(base_color_info) = pbr.base_color_texture() {
                let base_coord = base_color_info.tex_coord();
                *texcoord_base_counts.entry(base_coord).or_insert(0) += 1;
                if let Some(transform) = base_color_info.texture_transform() {
                    khr_texture_transform_primitives += 1;
                    if let Some(override_coord) = transform.tex_coord() {
                        *texcoord_override_counts.entry(override_coord).or_insert(0) += 1;
                    }
                }
            }
            if material.normal_texture().is_some() {
                normal_texture_primitives = normal_texture_primitives.saturating_add(1);
            }
            if material.emissive_texture().is_some() {
                emissive_texture_primitives = emissive_texture_primitives.saturating_add(1);
            }
            if material.occlusion_texture().is_some() {
                occlusion_texture_primitives = occlusion_texture_primitives.saturating_add(1);
            }
            if pbr.metallic_roughness_texture().is_some() {
                metallic_roughness_texture_primitives =
                    metallic_roughness_texture_primitives.saturating_add(1);
            }
            if material.double_sided() {
                double_sided_materials = double_sided_materials.saturating_add(1);
            }
        }
    }

    println!("file: {}", args.glb.display());
    println!(
        "extensions_required: {}",
        if extensions_required.is_empty() {
            "[]".to_owned()
        } else {
            format!("{extensions_required:?}")
        }
    );
    println!(
        "extensions_used: {}",
        if extensions_used.is_empty() {
            "[]".to_owned()
        } else {
            format!("{extensions_used:?}")
        }
    );
    println!(
        "unsupported_required_extensions: {}",
        if unsupported_required_extensions.is_empty() {
            "[]".to_owned()
        } else {
            format!("{unsupported_required_extensions:?}")
        }
    );
    println!(
        "unsupported_used_extensions: {}",
        if unsupported_used_extensions.is_empty() {
            "[]".to_owned()
        } else {
            format!("{unsupported_used_extensions:?}")
        }
    );
    println!(
        "khr_texture_transform_primitives: {}",
        khr_texture_transform_primitives
    );
    println!(
        "base_color_texcoord_distribution: {}",
        if texcoord_base_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texcoord_base_counts:?}")
        }
    );
    println!(
        "texcoord_override_distribution: {}",
        if texcoord_override_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texcoord_override_counts:?}")
        }
    );
    println!("non_triangle_primitives: {}", non_triangle_primitives);
    println!("normal_texture_primitives: {}", normal_texture_primitives);
    println!(
        "emissive_texture_primitives: {}",
        emissive_texture_primitives
    );
    println!(
        "occlusion_texture_primitives: {}",
        occlusion_texture_primitives
    );
    println!(
        "metallic_roughness_texture_primitives: {}",
        metallic_roughness_texture_primitives
    );
    println!("double_sided_materials: {}", double_sided_materials);
    println!("meshes: {}", scene.meshes.len());
    println!("mesh_instances: {}", scene.mesh_instances.len());
    println!("nodes: {}", scene.nodes.len());
    if let Some(root_idx) = scene.root_center_node {
        let root_name = scene
            .nodes
            .get(root_idx)
            .and_then(|node| node.name.as_deref())
            .unwrap_or("<unnamed>");
        println!("root_center_node: {} ({})", root_idx, root_name);
    } else {
        println!("root_center_node: none");
    }
    println!("skins: {}", scene.skins.len());
    println!("materials: {}", scene.materials.len());
    println!("textures: {}", scene.textures.len());
    let fallback_white_textures = scene
        .textures
        .iter()
        .filter(|texture| texture.source_format == "FallbackWhite")
        .count();
    println!("fallback_white_textures: {}", fallback_white_textures);
    println!(
        "renderer_material_coverage: baseColor/alpha/vertexColor/textureTransform only; normal/emissive/occlusion/PBR lighting are ignored by the terminal renderer"
    );
    println!("animations: {}", scene.animations.len());
    let mut texture_format_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut texture_color_space_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for texture in &scene.textures {
        *texture_format_counts
            .entry(texture.source_format.clone())
            .or_insert(0) += 1;
        let key = match texture.color_space {
            crate::scene::TextureColorSpace::Srgb => "sRGB",
            crate::scene::TextureColorSpace::Linear => "Linear",
        };
        *texture_color_space_counts.entry(key).or_insert(0) += 1;
    }
    let mut base_color_sampler_counts: BTreeMap<String, usize> = BTreeMap::new();
    for material in &scene.materials {
        let key = format!(
            "wrap=({:?},{:?}) filter=({:?},{:?})",
            material.base_color_wrap_s,
            material.base_color_wrap_t,
            material.base_color_min_filter,
            material.base_color_mag_filter
        );
        *base_color_sampler_counts.entry(key).or_insert(0) += 1;
    }
    println!(
        "texture_formats: {}",
        if texture_format_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texture_format_counts:?}")
        }
    );
    println!(
        "texture_color_spaces: {}",
        if texture_color_space_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texture_color_space_counts:?}")
        }
    );
    println!(
        "base_color_sampler_distribution: {}",
        if base_color_sampler_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{base_color_sampler_counts:?}")
        }
    );
    for (index, texture) in scene.textures.iter().enumerate() {
        let color_space = match texture.color_space {
            crate::scene::TextureColorSpace::Srgb => "sRGB",
            crate::scene::TextureColorSpace::Linear => "Linear",
        };
        println!(
            "texture[{index}]: {}x{} format={} color_space={} mips={}",
            texture.width,
            texture.height,
            texture.source_format,
            color_space,
            texture.mip_levels.len()
        );
    }
    for (index, material) in scene.materials.iter().enumerate() {
        println!(
            "material[{index}]: base_tex={:?} texcoord={} wrap=({:?},{:?}) filter=({:?},{:?}) alpha={:?} cutoff={:.3} double_sided={}",
            material.base_color_texture,
            material.base_color_tex_coord,
            material.base_color_wrap_s,
            material.base_color_wrap_t,
            material.base_color_min_filter,
            material.base_color_mag_filter,
            material.alpha_mode,
            material.alpha_cutoff,
            material.double_sided
        );
    }
    let total_morph_targets: usize = scene
        .meshes
        .iter()
        .map(|mesh| mesh.morph_targets.len())
        .sum();
    let weighted_instances = scene
        .mesh_instances
        .iter()
        .filter(|instance| !instance.default_morph_weights.is_empty())
        .count();
    println!("morph_targets: {}", total_morph_targets);
    println!("morph_weighted_instances: {}", weighted_instances);
    let vertex_color_primitives = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.colors_rgba.as_ref().is_some_and(|c| !c.is_empty()))
        .count();
    let uv_primitives = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.uv0.as_ref().is_some_and(|u| !u.is_empty()))
        .count();
    println!("vertex_color_primitives: {}", vertex_color_primitives);
    println!("uv_primitives: {}", uv_primitives);
    println!("total_vertices: {}", scene.total_vertices());
    println!("total_triangles: {}", scene.total_triangles());
    println!("total_joints: {}", scene.total_joints());
    if let Some(stats) = scene_stats_world(&scene) {
        let extent = (stats.max - stats.min).abs();
        let framing = compute_scene_framing(&scene, RenderConfig::default().fov_deg, 0.0, 0.0, 0.0);
        println!(
            "robust_bounds_min: [{:.4}, {:.4}, {:.4}]",
            stats.min.x, stats.min.y, stats.min.z
        );
        println!(
            "robust_bounds_max: [{:.4}, {:.4}, {:.4}]",
            stats.max.x, stats.max.y, stats.max.z
        );
        println!(
            "robust_extent: [{:.4}, {:.4}, {:.4}]",
            extent.x, extent.y, extent.z
        );
        println!(
            "median_center: [{:.4}, {:.4}, {:.4}]",
            stats.median.x, stats.median.y, stats.median.z
        );
        println!("distance_p90: {:.4}", stats.p90_distance);
        println!("distance_p98: {:.4}", stats.p98_distance);
        println!(
            "auto_frame: focus=[{:.4}, {:.4}, {:.4}] radius={:.4} camera_height={:.4}",
            framing.focus.x,
            framing.focus.y,
            framing.focus.z,
            framing.radius,
            framing.camera_height
        );
    }
    for (index, animation) in scene.animations.iter().enumerate() {
        let mut t_count = 0usize;
        let mut r_count = 0usize;
        let mut s_count = 0usize;
        let mut m_count = 0usize;
        for channel in &animation.channels {
            match channel.target {
                ChannelTarget::Translation => t_count += 1,
                ChannelTarget::Rotation => r_count += 1,
                ChannelTarget::Scale => s_count += 1,
                ChannelTarget::MorphWeights => m_count += 1,
                ChannelTarget::MaterialMorphWeights => m_count += 1,
            }
        }
        println!(
            "animation[{index}]: name={} duration={:.3}s channels={} (t/r/s/m={}/{}/{}/{})",
            animation.name.as_deref().unwrap_or("<unnamed>"),
            animation.duration,
            animation.channels.len(),
            t_count,
            r_count,
            s_count,
            m_count
        );
    }
    Ok(())
}
