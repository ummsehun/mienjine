use image::{ImageBuffer, Rgba, imageops};

use crate::animation::Interpolation;
use crate::scene::{
    TextureColorSpace, TextureCpu, TextureFilterMode, TextureLevelCpu, TextureWrapMode,
    UvTransform2D,
};

pub(super) fn convert_texture_transform(info: &gltf::texture::Info<'_>) -> Option<UvTransform2D> {
    let transform = info.texture_transform()?;
    Some(UvTransform2D {
        offset: transform.offset(),
        scale: transform.scale(),
        rotation_rad: transform.rotation(),
        tex_coord_override: transform.tex_coord(),
    })
}

pub(super) fn map_wrap_mode(value: gltf::texture::WrappingMode) -> TextureWrapMode {
    match value {
        gltf::texture::WrappingMode::Repeat => TextureWrapMode::Repeat,
        gltf::texture::WrappingMode::MirroredRepeat => TextureWrapMode::MirroredRepeat,
        gltf::texture::WrappingMode::ClampToEdge => TextureWrapMode::ClampToEdge,
    }
}

pub(super) fn map_min_filter(value: Option<gltf::texture::MinFilter>) -> TextureFilterMode {
    match value.unwrap_or(gltf::texture::MinFilter::LinearMipmapLinear) {
        gltf::texture::MinFilter::Nearest
        | gltf::texture::MinFilter::NearestMipmapNearest
        | gltf::texture::MinFilter::NearestMipmapLinear => TextureFilterMode::Nearest,
        gltf::texture::MinFilter::Linear
        | gltf::texture::MinFilter::LinearMipmapNearest
        | gltf::texture::MinFilter::LinearMipmapLinear => TextureFilterMode::Linear,
    }
}

pub(super) fn map_mag_filter(value: Option<gltf::texture::MagFilter>) -> TextureFilterMode {
    match value.unwrap_or(gltf::texture::MagFilter::Linear) {
        gltf::texture::MagFilter::Nearest => TextureFilterMode::Nearest,
        gltf::texture::MagFilter::Linear => TextureFilterMode::Linear,
    }
}

pub(super) fn classify_texture_color_spaces(
    document: &gltf::Document,
    texture_count: usize,
) -> Vec<TextureColorSpace> {
    let mut srgb_hits = vec![0usize; texture_count];
    let mut linear_hits = vec![0usize; texture_count];
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        if let Some(info) = pbr.base_color_texture() {
            let index = info.texture().source().index();
            if index < texture_count {
                srgb_hits[index] = srgb_hits[index].saturating_add(1);
            }
        }
        if let Some(info) = material.emissive_texture() {
            let index = info.texture().source().index();
            if index < texture_count {
                srgb_hits[index] = srgb_hits[index].saturating_add(1);
            }
        }
        if let Some(info) = material.normal_texture() {
            let index = info.texture().source().index();
            if index < texture_count {
                linear_hits[index] = linear_hits[index].saturating_add(1);
            }
        }
        if let Some(info) = material.occlusion_texture() {
            let index = info.texture().source().index();
            if index < texture_count {
                linear_hits[index] = linear_hits[index].saturating_add(1);
            }
        }
        if let Some(info) = pbr.metallic_roughness_texture() {
            let index = info.texture().source().index();
            if index < texture_count {
                linear_hits[index] = linear_hits[index].saturating_add(1);
            }
        }
    }
    (0..texture_count)
        .map(|index| {
            if srgb_hits[index] == 0 && linear_hits[index] > 0 {
                TextureColorSpace::Linear
            } else {
                TextureColorSpace::Srgb
            }
        })
        .collect()
}

pub(super) fn map_interpolation(value: gltf::animation::Interpolation) -> Interpolation {
    match value {
        gltf::animation::Interpolation::Linear => Interpolation::Linear,
        gltf::animation::Interpolation::Step => Interpolation::Step,
        gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
    }
}

pub(super) fn resolve_default_morph_weights(
    node_weights: Option<&[f32]>,
    mesh_weights: Option<&[f32]>,
    target_count: usize,
) -> Vec<f32> {
    if target_count == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0; target_count];
    if let Some(source) = node_weights.or(mesh_weights) {
        for (i, value) in source.iter().take(target_count).enumerate() {
            out[i] = *value;
        }
    }
    out
}

pub(super) fn infer_morph_weights_per_key(
    hinted_count: usize,
    output_values_len: usize,
    keyframe_count: usize,
    interpolation: Interpolation,
) -> usize {
    if keyframe_count == 0 || output_values_len == 0 {
        return 0;
    }
    let spline_factor = if interpolation == Interpolation::CubicSpline {
        3
    } else {
        1
    };
    let denom = keyframe_count.saturating_mul(spline_factor);
    if denom == 0 {
        return 0;
    }
    let inferred = output_values_len / denom;
    let mut weights_per_key = if hinted_count > 0 {
        hinted_count
    } else {
        inferred
    };
    if weights_per_key == 0 {
        return 0;
    }
    let expected = keyframe_count
        .saturating_mul(spline_factor)
        .saturating_mul(weights_per_key);
    if output_values_len < expected {
        weights_per_key = inferred;
    }
    weights_per_key
}

pub(super) fn convert_image_to_texture(image: &gltf::image::Data) -> Option<TextureCpu> {
    use gltf::image::Format;

    let pixels = match image.format {
        Format::R8 => image
            .pixels
            .iter()
            .flat_map(|r| [*r, *r, *r, 255])
            .collect::<Vec<_>>(),
        Format::R8G8 => image
            .pixels
            .chunks_exact(2)
            .flat_map(|px| [px[0], px[1], 0, 255])
            .collect::<Vec<_>>(),
        Format::R8G8B8 => image
            .pixels
            .chunks_exact(3)
            .flat_map(|px| [px[0], px[1], px[2], 255])
            .collect::<Vec<_>>(),
        Format::R8G8B8A8 => image.pixels.clone(),
        _ => return None,
    };
    let mip_levels = build_mip_levels(image.width, image.height, &pixels, 5);
    Some(TextureCpu {
        width: image.width,
        height: image.height,
        rgba8: pixels,
        source_format: format!("{:?}", image.format),
        color_space: TextureColorSpace::Srgb,
        mip_levels,
    })
}

pub(super) fn fallback_white_texture() -> TextureCpu {
    TextureCpu {
        width: 1,
        height: 1,
        rgba8: vec![255, 255, 255, 255],
        source_format: "FallbackWhite".to_owned(),
        color_space: TextureColorSpace::Srgb,
        mip_levels: Vec::new(),
    }
}

pub(super) fn build_mip_levels(
    base_width: u32,
    base_height: u32,
    base_rgba: &[u8],
    max_levels: usize,
) -> Vec<TextureLevelCpu> {
    if base_width == 0 || base_height == 0 || max_levels <= 1 {
        return Vec::new();
    }
    let mut levels = Vec::new();
    let Some(mut current) =
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(base_width, base_height, base_rgba.to_vec())
    else {
        return levels;
    };
    for _ in 1..max_levels {
        let next_w = (current.width() / 2).max(1);
        let next_h = (current.height() / 2).max(1);
        if next_w == current.width() && next_h == current.height() {
            break;
        }
        let resized = imageops::resize(&current, next_w, next_h, imageops::FilterType::Triangle);
        levels.push(TextureLevelCpu {
            width: next_w,
            height: next_h,
            rgba8: resized.as_raw().clone(),
        });
        current = resized;
    }
    levels
}

pub(super) fn load_pmx_texture(path: &std::path::Path, name: &str, _index: usize) -> TextureCpu {
    if path.exists() {
        match image::open(path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let pixels = rgba.into_raw();
                let mip_levels = build_mip_levels(width, height, &pixels, 5);
                return TextureCpu {
                    width,
                    height,
                    rgba8: pixels,
                    source_format: format!("PMXTexture({})", name),
                    color_space: TextureColorSpace::Srgb,
                    mip_levels,
                };
            }
            Err(e) => {
                crate::shared::pmx_log::warn(format!(
                    "warning: failed to load PMX texture '{}': {}. using fallback.",
                    path.display(),
                    e
                ));
            }
        }
    }
    fallback_white_texture()
}
