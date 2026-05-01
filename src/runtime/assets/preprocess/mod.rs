use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::runtime::cli::{PreprocessArgs, PreprocessPresetArg};

mod image_ops;

use self::image_ops::{
    ImageColorSpace, PreprocessReport, SharpenPolicy, classify_image_color_spaces,
    extract_image_source_bytes, set_image_as_data_uri_png, upscale_image_bytes,
};

pub fn run_preprocess(args: &PreprocessArgs) -> Result<()> {
    let mut factor = match args.upscale_factor {
        1 | 2 | 4 => args.upscale_factor,
        _ => bail!(
            "unsupported --upscale-factor {} (allowed: 1,2,4)",
            args.upscale_factor
        ),
    };
    let mut sharpen = args.upscale_sharpen.clamp(0.0, 2.0);
    let sharpen_policy = if matches!(args.preset, PreprocessPresetArg::FacePriority) {
        SharpenPolicy::SrgbOnly
    } else {
        SharpenPolicy::All
    };
    if matches!(args.preset, PreprocessPresetArg::WebParity) && factor == 2 {
        // Keep authored texture intent by default in web-parity mode.
        factor = 1;
        sharpen = 0.0;
    }
    if !args.glb.exists() {
        bail!("input file not found: {}", args.glb.display());
    }
    if !args
        .glb
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"))
    {
        bail!(
            "preprocess currently supports .glb input only: {}",
            args.glb.display()
        );
    }

    let input_bytes = fs::read(&args.glb)
        .with_context(|| format!("failed to read input GLB: {}", args.glb.display()))?;
    let glb = gltf::binary::Glb::from_slice(&input_bytes)
        .with_context(|| format!("failed to parse GLB container: {}", args.glb.display()))?;

    let mut json: Value = serde_json::from_slice(glb.json.as_ref())
        .context("failed to parse GLB JSON chunk as JSON")?;
    let color_spaces = classify_image_color_spaces(&json);
    let input_parent = args
        .glb
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut report = PreprocessReport::default();
    let json_snapshot = json.clone();
    if let Some(images) = json.get_mut("images").and_then(Value::as_array_mut) {
        report.images_total = images.len();
        for (index, image) in images.iter_mut().enumerate() {
            let source = extract_image_source_bytes(
                image,
                &json_snapshot,
                glb.bin.as_deref(),
                &input_parent,
            );
            let Ok(source_bytes) = source else {
                report.images_failed = report.images_failed.saturating_add(1);
                eprintln!("warning: preprocess skipped image[{index}] due to unsupported source");
                continue;
            };
            let color_space = color_spaces
                .get(index)
                .copied()
                .unwrap_or(ImageColorSpace::Srgb);
            match upscale_image_bytes(&source_bytes, factor, sharpen, color_space, sharpen_policy) {
                Ok(png_bytes) => {
                    set_image_as_data_uri_png(image, &png_bytes);
                    report.images_upscaled = report.images_upscaled.saturating_add(1);
                }
                Err(err) => {
                    report.images_failed = report.images_failed.saturating_add(1);
                    eprintln!("warning: preprocess failed image[{index}]: {err}");
                }
            }
        }
    }

    let json_bytes = serde_json::to_vec(&json).context("failed to serialize updated GLB JSON")?;
    let out_glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(json_bytes),
        bin: glb.bin.map(|bin| Cow::Owned(bin.into_owned())),
    };
    let bytes = out_glb
        .to_vec()
        .context("failed to encode output GLB container")?;
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory: {}", parent.display()))?;
    }
    fs::write(&args.out, bytes)
        .with_context(|| format!("failed to write output GLB: {}", args.out.display()))?;

    println!("preprocess input: {}", args.glb.display());
    println!("preprocess output: {}", args.out.display());
    println!("preset: {:?}", args.preset);
    println!("images_total: {}", report.images_total);
    println!("images_upscaled: {}", report.images_upscaled);
    println!("images_failed: {}", report.images_failed);
    println!("upscale_factor: {}", factor);
    println!("upscale_sharpen: {:.2}", sharpen);
    Ok(())
}
