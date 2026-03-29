//! Tests for the renderer module.

use glam::{Vec2, Vec3};

use crate::render::background::theme_palette;
use crate::render::renderer::braille::{braille_thresholds, compose_braille_cells};
use crate::render::renderer::shading::contrast_params;
use crate::render::renderer::{
    encode_ansi_frame, exposure_bias_multiplier, BrailleSubpixelBuffers, FrameBuffers, GlyphRamp,
};
use crate::render::renderer_glyph::{glyph_for_intensity, select_charset};
use crate::render::renderer_material::sample_material;
use crate::render::renderer_metrics::visible_cell_ratio;
use crate::render::renderer_texture::{prefer_sampling_for_focus, select_mip_level};
use crate::scene::{
    AnsiQuantization, BrailleProfile, CameraFocusMode, CellAspectMode, ColorMode,
    MaterialAlphaMode, MaterialCpu, MaterialToonSource, RenderConfig, RenderMode,
    TextureColorSpace, TextureCpu, TextureFilterMode, TextureLevelCpu, TextureSamplingMode,
    TextureWrapMode, ThemeStyle,
};

#[test]
fn adaptive_contrast_is_in_range() {
    let config = RenderConfig::default();
    for cells in [4_000, 8_000, 20_000] {
        let params = contrast_params(&config, cells);
        assert!((0.0..=0.4).contains(&params.floor));
        assert!((0.5..=1.3).contains(&params.gamma));
        assert!((0.2..=1.5).contains(&params.fog_scale));
    }
}

#[test]
fn glyph_for_intensity_is_monotonic() {
    let glyph_ramp = GlyphRamp::from_config(&RenderConfig::default());
    let charset = glyph_ramp.chars();
    let low = glyph_for_intensity(0.2, charset);
    let high = glyph_for_intensity(0.8, charset);
    let idx_low = charset.iter().position(|c| *c == low).unwrap_or_default();
    let idx_high = charset.iter().position(|c| *c == high).unwrap_or_default();
    assert!(idx_high >= idx_low);
}

#[test]
fn adaptive_charset_uses_default_only() {
    let mut config = RenderConfig {
        cell_aspect_mode: CellAspectMode::Auto,
        ..RenderConfig::default()
    };
    let fallback = vec!['.', '#'];
    let picked = select_charset(&config, &fallback, 5_000);
    assert_ne!(picked, fallback.as_slice());

    config.charset = " .:-=+*#%@X".to_owned();
    let picked_custom = select_charset(&config, &fallback, 5_000);
    assert_eq!(picked_custom, fallback.as_slice());
}

#[test]
fn braille_safe_guard_promotes_non_empty_cell() {
    let mut frame = FrameBuffers::new(1, 1);
    let mut sub = BrailleSubpixelBuffers::default();
    sub.resize(2, 4);
    sub.clear();
    sub.intensity[0] = 0.07;
    sub.color_rgb[0] = [220, 180, 120];
    sub.depth[0] = 0.42;
    let config = RenderConfig {
        mode: RenderMode::Braille,
        color_mode: ColorMode::Ansi,
        braille_profile: BrailleProfile::Safe,
        theme_style: ThemeStyle::Theater,
        ..RenderConfig::default()
    };
    compose_braille_cells(
        &mut frame,
        &sub,
        &config,
        theme_palette(config.theme_style),
        braille_thresholds(config.braille_profile, config.clarity_profile, false),
    );
    assert_ne!(frame.glyphs[0], ' ');
    assert_ne!(frame.glyphs[0], '⠀');
}

#[test]
fn ansi_encoder_emits_escape_sequences() {
    let mut frame = FrameBuffers::new(2, 1);
    frame.has_color = true;
    frame.glyphs[0] = '@';
    frame.glyphs[1] = '#';
    frame.fg_rgb[0] = [255, 0, 0];
    frame.fg_rgb[1] = [0, 255, 0];
    let mut out = String::new();
    encode_ansi_frame(&frame, &mut out, AnsiQuantization::Off);
    assert!(out.contains("\x1b[38;2;255;0;0m"));
    assert!(out.contains("\x1b[38;2;0;255;0m"));
    assert!(out.contains("@"));
    assert!(out.contains("#"));
}

#[test]
fn exposure_bias_multiplier_is_monotonic() {
    let low = exposure_bias_multiplier(-0.4);
    let mid = exposure_bias_multiplier(0.0);
    let high = exposure_bias_multiplier(0.6);
    assert!(low < mid);
    assert!(mid < high);
}

#[test]
fn visible_ratio_uses_depth_buffer() {
    let mut frame = FrameBuffers::new(4, 1);
    frame.depth = vec![f32::INFINITY, 0.3, f32::INFINITY, 0.7];
    let ratio = visible_cell_ratio(&frame);
    assert!((ratio - 0.5).abs() < 1e-6);
}

#[test]
fn face_focus_promotes_bilinear_sampling() {
    assert!(matches!(
        prefer_sampling_for_focus(TextureSamplingMode::Nearest, CameraFocusMode::Face),
        TextureSamplingMode::Bilinear
    ));
    assert!(matches!(
        prefer_sampling_for_focus(TextureSamplingMode::Nearest, CameraFocusMode::Upper),
        TextureSamplingMode::Bilinear
    ));
    assert!(matches!(
        prefer_sampling_for_focus(TextureSamplingMode::Nearest, CameraFocusMode::Auto),
        TextureSamplingMode::Nearest
    ));
}

#[test]
fn face_focus_prefers_sharper_mip_levels() {
    let texture = TextureCpu {
        width: 4,
        height: 4,
        rgba8: vec![255; 4 * 4 * 4],
        source_format: "png".to_owned(),
        color_space: TextureColorSpace::Srgb,
        mip_levels: vec![
            TextureLevelCpu {
                width: 2,
                height: 2,
                rgba8: vec![200; 2 * 2 * 4],
            },
            TextureLevelCpu {
                width: 1,
                height: 1,
                rgba8: vec![120; 4],
            },
        ],
    };
    let base = select_mip_level(&texture, 0.85, 0.0, CameraFocusMode::Auto);
    let face = select_mip_level(&texture, 0.85, 0.0, CameraFocusMode::Face);
    assert!(face <= base);
}

fn sample_scene_with_texture(
    texel: [u8; 4],
    color_space: TextureColorSpace,
) -> crate::scene::SceneCpu {
    let material = MaterialCpu {
        base_color_factor: [1.0, 1.0, 1.0, 1.0],
        base_color_texture: Some(0),
        base_color_tex_coord: 0,
        base_color_uv_transform: None,
        base_color_wrap_s: TextureWrapMode::Repeat,
        base_color_wrap_t: TextureWrapMode::Repeat,
        base_color_min_filter: TextureFilterMode::Linear,
        base_color_mag_filter: TextureFilterMode::Linear,
        sphere_texture: None,
        toon_source: None,
        emissive_factor: [0.0, 0.0, 0.0],
        alpha_mode: MaterialAlphaMode::Opaque,
        alpha_cutoff: 0.5,
        double_sided: false,
    };
    let texture = TextureCpu {
        width: 1,
        height: 1,
        rgba8: texel.to_vec(),
        source_format: "png".to_owned(),
        color_space,
        mip_levels: Vec::new(),
    };
    crate::scene::SceneCpu {
        materials: vec![material],
        textures: vec![texture],
        ..crate::scene::SceneCpu::default()
    }
}

#[test]
fn material_sampling_respects_texture_color_space() {
    let linear_scene = sample_scene_with_texture([128, 128, 128, 200], TextureColorSpace::Linear);
    let srgb_scene = sample_scene_with_texture([128, 128, 128, 200], TextureColorSpace::Srgb);
    let cfg = RenderConfig::default();

    let sampled_linear = sample_material(
        &linear_scene,
        Some(0),
        Vec2::ZERO,
        Vec2::ZERO,
        Vec3::Y,
        0.5,
        0.2,
        [1.0, 1.0, 1.0, 1.0],
        &cfg,
        &[],
    );
    let sampled_srgb = sample_material(
        &srgb_scene,
        Some(0),
        Vec2::ZERO,
        Vec2::ZERO,
        Vec3::Y,
        0.5,
        0.2,
        [1.0, 1.0, 1.0, 1.0],
        &cfg,
        &[],
    );

    assert!(sampled_srgb.albedo_linear[0] < sampled_linear.albedo_linear[0]);
    assert!((sampled_linear.alpha - sampled_srgb.alpha).abs() < 1e-6);
}

#[test]
fn material_sampling_clamps_alpha_to_unit_interval() {
    let mut scene = sample_scene_with_texture([200, 200, 200, 255], TextureColorSpace::Linear);
    scene.materials[0].base_color_factor[3] = 2.0;
    let cfg = RenderConfig::default();
    let sampled = sample_material(
        &scene,
        Some(0),
        Vec2::ZERO,
        Vec2::ZERO,
        Vec3::Y,
        0.5,
        0.2,
        [1.0, 1.0, 1.5, 2.0],
        &cfg,
        &[],
    );
    assert!((0.0..=1.0).contains(&sampled.alpha));
    assert!((sampled.alpha - 1.0).abs() < 1e-6);
}

#[test]
fn material_sampling_applies_sphere_texture_and_builtin_toon() {
    let sphere_scene = crate::scene::SceneCpu {
        materials: vec![MaterialCpu {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            base_color_tex_coord: 0,
            base_color_uv_transform: None,
            base_color_wrap_s: TextureWrapMode::Repeat,
            base_color_wrap_t: TextureWrapMode::Repeat,
            base_color_min_filter: TextureFilterMode::Linear,
            base_color_mag_filter: TextureFilterMode::Linear,
            sphere_texture: Some(0),
            toon_source: Some(MaterialToonSource::BuiltIn(0)),
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: MaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }],
        textures: vec![TextureCpu {
            width: 1,
            height: 1,
            rgba8: vec![64, 128, 255, 255],
            source_format: "png".to_owned(),
            color_space: TextureColorSpace::Srgb,
            mip_levels: Vec::new(),
        }],
        ..crate::scene::SceneCpu::default()
    };
    let cfg = RenderConfig::default();

    let sampled = sample_material(
        &sphere_scene,
        Some(0),
        Vec2::ZERO,
        Vec2::ZERO,
        Vec3::new(0.3, 0.8, 0.5),
        0.7,
        0.2,
        [1.0, 1.0, 1.0, 1.0],
        &cfg,
        &[],
    );

    assert!(sampled.albedo_linear[0] < 1.0);
    assert!(sampled.albedo_linear[2] < 1.0);
}
