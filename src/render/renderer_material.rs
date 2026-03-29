use glam::Vec2;

use crate::render::material_morph::apply_material_morph_to_index;
use crate::scene::{
    MaterialAlphaMode, MaterialCpu, RenderConfig, SceneCpu, TextureColorSpace, TextureFilterMode,
    TextureSamplerMode, TextureSamplingMode,
};

use super::renderer_color::srgb_to_linear;
use super::renderer_texture::{
    apply_uv_transform, prefer_sampling_for_focus, sample_texture_rgba, select_mip_level,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct MaterialSample {
    pub(super) albedo_linear: [f32; 3],
    pub(super) alpha: f32,
    pub(super) emissive_linear: [f32; 3],
    pub(super) alpha_mode: MaterialAlphaMode,
    pub(super) alpha_cutoff: f32,
    pub(super) double_sided: bool,
}

pub(super) fn resolve_material_props(
    scene: &SceneCpu,
    material_index: Option<usize>,
) -> MaterialSample {
    if let Some(material) = material_index.and_then(|index| scene.materials.get(index)) {
        return MaterialSample {
            albedo_linear: [1.0, 1.0, 1.0],
            alpha: 1.0,
            emissive_linear: [
                material.emissive_factor[0].clamp(0.0, 1.0),
                material.emissive_factor[1].clamp(0.0, 1.0),
                material.emissive_factor[2].clamp(0.0, 1.0),
            ],
            alpha_mode: material.alpha_mode,
            alpha_cutoff: material.alpha_cutoff.clamp(0.0, 1.0),
            double_sided: material.double_sided,
        };
    }
    MaterialSample {
        albedo_linear: [1.0, 1.0, 1.0],
        alpha: 1.0,
        emissive_linear: [0.0, 0.0, 0.0],
        alpha_mode: MaterialAlphaMode::Opaque,
        alpha_cutoff: 0.5,
        double_sided: false,
    }
}

pub(super) fn sample_material(
    scene: &SceneCpu,
    material_index: Option<usize>,
    uv0: Vec2,
    uv1: Vec2,
    depth: f32,
    vertex_color: [f32; 4],
    config: &RenderConfig,
    material_morph_weights: &[f32],
) -> MaterialSample {
    if !config.material_color {
        let mut material = resolve_material_props(scene, material_index);
        material.albedo_linear = [1.0, 1.0, 1.0];
        material.alpha = 1.0;
        return material;
    }
    let base_material = if let Some(index) = material_index {
        if let Some(mat) = scene.materials.get(index) {
            Some(apply_material_morph_to_index(
                mat,
                index,
                &scene.material_morphs,
                material_morph_weights,
            ))
        } else {
            None
        }
    } else {
        None
    };
    let mut out = resolve_material_props(scene, material_index);
    let mut color = [
        vertex_color[0],
        vertex_color[1],
        vertex_color[2],
        vertex_color[3],
    ];
    if let Some(ref material) = base_material {
        color[0] *= material.base_color_factor[0];
        color[1] *= material.base_color_factor[1];
        color[2] *= material.base_color_factor[2];
        color[3] *= material.base_color_factor[3];
        if let Some(texture_index) = material.base_color_texture {
            if let Some(texture) = scene.textures.get(texture_index) {
                let mut selected_uv = match material
                    .base_color_uv_transform
                    .and_then(|transform| transform.tex_coord_override)
                    .unwrap_or(material.base_color_tex_coord)
                {
                    0 => uv0,
                    1 => uv1,
                    _ => uv0,
                };
                if let Some(transform) = material.base_color_uv_transform {
                    selected_uv = apply_uv_transform(selected_uv, transform);
                }
                let sampling_mode = match config.texture_sampler {
                    TextureSamplerMode::Override => config.texture_sampling,
                    TextureSamplerMode::Gltf => {
                        if matches!(material.base_color_mag_filter, TextureFilterMode::Nearest)
                            || matches!(material.base_color_min_filter, TextureFilterMode::Nearest)
                        {
                            TextureSamplingMode::Nearest
                        } else {
                            TextureSamplingMode::Bilinear
                        }
                    }
                };
                let sampling_mode = prefer_sampling_for_focus(sampling_mode, config.camera_focus);
                let mip_level =
                    select_mip_level(texture, depth, config.texture_mip_bias, config.camera_focus);
                let sampled = sample_texture_rgba(
                    texture,
                    selected_uv,
                    sampling_mode,
                    config.texture_v_origin,
                    material.base_color_wrap_s,
                    material.base_color_wrap_t,
                    mip_level,
                );
                let sample_rgb = match texture.color_space {
                    TextureColorSpace::Srgb => [
                        srgb_to_linear(sampled[0]),
                        srgb_to_linear(sampled[1]),
                        srgb_to_linear(sampled[2]),
                    ],
                    TextureColorSpace::Linear => [sampled[0], sampled[1], sampled[2]],
                };
                color[0] *= sample_rgb[0];
                color[1] *= sample_rgb[1];
                color[2] *= sample_rgb[2];
                color[3] *= sampled[3];
            }
        }
    }
    out.albedo_linear = [
        color[0].clamp(0.0, 1.0),
        color[1].clamp(0.0, 1.0),
        color[2].clamp(0.0, 1.0),
    ];
    out.alpha = color[3].clamp(0.0, 1.0);
    out
}
