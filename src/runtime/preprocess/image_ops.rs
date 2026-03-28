use std::{fs, io::Cursor, path::Path};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba, RgbaImage};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImageColorSpace {
    Srgb,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharpenPolicy {
    All,
    SrgbOnly,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct PreprocessReport {
    pub(super) images_total: usize,
    pub(super) images_upscaled: usize,
    pub(super) images_failed: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct UsageCounter {
    srgb_hits: usize,
    linear_hits: usize,
}

pub(super) fn classify_image_color_spaces(root: &Value) -> Vec<ImageColorSpace> {
    let image_count = root
        .get("images")
        .and_then(Value::as_array)
        .map(|images| images.len())
        .unwrap_or(0);
    let mut usage = vec![UsageCounter::default(); image_count];

    let texture_sources = build_texture_source_lookup(root);
    if let Some(materials) = root.get("materials").and_then(Value::as_array) {
        for material in materials {
            if let Some(index) =
                texture_index(material.pointer("/pbrMetallicRoughness/baseColorTexture/index"))
            {
                mark_usage(&mut usage, &texture_sources, index, ImageColorSpace::Srgb);
            }
            if let Some(index) = texture_index(material.pointer("/emissiveTexture/index")) {
                mark_usage(&mut usage, &texture_sources, index, ImageColorSpace::Srgb);
            }
            if let Some(index) = texture_index(material.pointer("/normalTexture/index")) {
                mark_usage(&mut usage, &texture_sources, index, ImageColorSpace::Linear);
            }
            if let Some(index) = texture_index(material.pointer("/occlusionTexture/index")) {
                mark_usage(&mut usage, &texture_sources, index, ImageColorSpace::Linear);
            }
            if let Some(index) = texture_index(
                material.pointer("/pbrMetallicRoughness/metallicRoughnessTexture/index"),
            ) {
                mark_usage(&mut usage, &texture_sources, index, ImageColorSpace::Linear);
            }
        }
    }

    usage
        .into_iter()
        .map(|counter| {
            if counter.srgb_hits == 0 && counter.linear_hits > 0 {
                ImageColorSpace::Linear
            } else {
                ImageColorSpace::Srgb
            }
        })
        .collect()
}

fn build_texture_source_lookup(root: &Value) -> Vec<Option<usize>> {
    root.get("textures")
        .and_then(Value::as_array)
        .map(|textures| {
            textures
                .iter()
                .map(|tex| {
                    tex.get("source")
                        .and_then(Value::as_u64)
                        .map(|value| value as usize)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn mark_usage(
    usage: &mut [UsageCounter],
    texture_sources: &[Option<usize>],
    texture_index: usize,
    color_space: ImageColorSpace,
) {
    let Some(Some(image_index)) = texture_sources.get(texture_index) else {
        return;
    };
    let Some(entry) = usage.get_mut(*image_index) else {
        return;
    };
    match color_space {
        ImageColorSpace::Srgb => entry.srgb_hits = entry.srgb_hits.saturating_add(1),
        ImageColorSpace::Linear => entry.linear_hits = entry.linear_hits.saturating_add(1),
    }
}

fn texture_index(node: Option<&Value>) -> Option<usize> {
    node.and_then(Value::as_u64).map(|value| value as usize)
}

pub(super) fn extract_image_source_bytes(
    image: &Value,
    root: &Value,
    bin_chunk: Option<&[u8]>,
    input_parent: &Path,
) -> Result<Vec<u8>> {
    if let Some(uri) = image.get("uri").and_then(Value::as_str) {
        if let Some(encoded) = uri.strip_prefix("data:") {
            return decode_data_uri(encoded);
        }
        let bytes = fs::read(input_parent.join(uri))
            .with_context(|| format!("failed to read external image URI: {uri}"))?;
        return Ok(bytes);
    }

    let view_index = image
        .get("bufferView")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .context("image has no uri and no bufferView")?;
    let view = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|views| views.get(view_index))
        .context("bufferView index out of range for image source")?;
    let byte_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let byte_length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .context("bufferView missing byteLength")?;
    let bin = bin_chunk.context("GLB has no BIN chunk for bufferView image source")?;
    if byte_offset.saturating_add(byte_length) > bin.len() {
        bail!(
            "bufferView image source out of range (offset={} length={} bin={})",
            byte_offset,
            byte_length,
            bin.len()
        );
    }
    Ok(bin[byte_offset..byte_offset + byte_length].to_vec())
}

fn decode_data_uri(uri_without_prefix: &str) -> Result<Vec<u8>> {
    let Some((meta, payload)) = uri_without_prefix.split_once(',') else {
        bail!("invalid data URI");
    };
    if meta.contains(";base64") {
        let decoded = STANDARD
            .decode(payload.as_bytes())
            .context("failed to decode base64 data URI payload")?;
        Ok(decoded)
    } else {
        bail!("non-base64 data URI is unsupported")
    }
}

pub(super) fn set_image_as_data_uri_png(image: &mut Value, png_bytes: &[u8]) {
    let encoded = STANDARD.encode(png_bytes);
    let uri = format!("data:image/png;base64,{encoded}");
    let Some(obj) = image.as_object_mut() else {
        return;
    };
    obj.insert("uri".to_owned(), Value::String(uri));
    obj.insert("mimeType".to_owned(), Value::String("image/png".to_owned()));
    obj.remove("bufferView");
}

pub(super) fn upscale_image_bytes(
    image_bytes: &[u8],
    factor: u32,
    sharpen: f32,
    color_space: ImageColorSpace,
    sharpen_policy: SharpenPolicy,
) -> Result<Vec<u8>> {
    let decoded = image::load_from_memory(image_bytes).context("failed to decode image bytes")?;
    let (src_w, src_h) = decoded.dimensions();
    let target_w = src_w.saturating_mul(factor.max(1)).max(1);
    let target_h = src_h.saturating_mul(factor.max(1)).max(1);

    let mut upscaled = if matches!(color_space, ImageColorSpace::Srgb) {
        resize_srgb_linear(decoded.to_rgba8(), target_w, target_h)
    } else {
        imageops::resize(
            &decoded.to_rgba8(),
            target_w,
            target_h,
            imageops::FilterType::Lanczos3,
        )
    };

    let should_sharpen = sharpen > f32::EPSILON
        && (matches!(sharpen_policy, SharpenPolicy::All)
            || matches!(color_space, ImageColorSpace::Srgb));
    if should_sharpen {
        let sigma = (sharpen * 2.0).clamp(0.1, 6.0);
        let dyn_image = DynamicImage::ImageRgba8(upscaled);
        upscaled = dyn_image.unsharpen(sigma, 1).to_rgba8();
    }

    let mut out = Vec::new();
    DynamicImage::ImageRgba8(upscaled)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .context("failed to encode PNG image")?;
    Ok(out)
}

fn resize_srgb_linear(src: RgbaImage, target_w: u32, target_h: u32) -> RgbaImage {
    let mut linear = ImageBuffer::<Rgba<f32>, Vec<f32>>::new(src.width(), src.height());
    for (x, y, px) in src.enumerate_pixels() {
        linear.put_pixel(
            x,
            y,
            Rgba([
                srgb_to_linear(px[0]),
                srgb_to_linear(px[1]),
                srgb_to_linear(px[2]),
                (px[3] as f32) / 255.0,
            ]),
        );
    }

    let resized = imageops::resize(
        &linear,
        target_w.max(1),
        target_h.max(1),
        imageops::FilterType::Lanczos3,
    );
    let mut out = RgbaImage::new(target_w.max(1), target_h.max(1));
    for (x, y, px) in resized.enumerate_pixels() {
        out.put_pixel(
            x,
            y,
            Rgba([
                linear_to_srgb(px[0]),
                linear_to_srgb(px[1]),
                linear_to_srgb(px[2]),
                (px[3].clamp(0.0, 1.0) * 255.0).round() as u8,
            ]),
        );
    }
    out
}

fn srgb_to_linear(c: u8) -> f32 {
    let v = (c as f32) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let srgb = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_base64_data_uri_payload() {
        let data = decode_data_uri("image/png;base64,aGVsbG8=").expect("decode");
        assert_eq!(data, b"hello");
    }

    #[test]
    fn face_priority_skips_sharpen_for_linear_textures() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([30, 80, 160, 255]));
        image.put_pixel(1, 0, Rgba([220, 30, 80, 255]));
        image.put_pixel(0, 1, Rgba([50, 200, 70, 255]));
        image.put_pixel(1, 1, Rgba([240, 240, 80, 255]));
        let mut src = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut src), ImageFormat::Png)
            .expect("encode src");

        let linear_a = upscale_image_bytes(
            &src,
            2,
            0.8,
            ImageColorSpace::Linear,
            SharpenPolicy::SrgbOnly,
        )
        .expect("linear a");
        let linear_b = upscale_image_bytes(
            &src,
            2,
            0.0,
            ImageColorSpace::Linear,
            SharpenPolicy::SrgbOnly,
        )
        .expect("linear b");
        assert_eq!(linear_a, linear_b);

        let srgb_a =
            upscale_image_bytes(&src, 2, 0.8, ImageColorSpace::Srgb, SharpenPolicy::SrgbOnly)
                .expect("srgb a");
        let srgb_b =
            upscale_image_bytes(&src, 2, 0.0, ImageColorSpace::Srgb, SharpenPolicy::SrgbOnly)
                .expect("srgb b");
        assert_ne!(srgb_a, srgb_b);
    }
}
