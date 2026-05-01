//! Parser for visual settings: color, output, kitty, stage, graphics, appearance.

use crate::runtime::config::types::GasciiConfig;
use crate::runtime::config_parse::parse_bool;
use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CinematicCameraMode, ClarityProfile,
    ColorMode, DetailProfile, GraphicsProtocol, KittyCompression, KittyInternalResPreset,
    KittyPipelineMode, KittyTransport, PerfProfile, RecoverStrategy, RenderBackend,
    RenderOutputMode, StageQuality, StageRole, ThemeStyle,
};

/// Parse `color_mode`.
pub fn parse_color_mode(value: &str) -> Option<ColorMode> {
    let lower = value.to_ascii_lowercase();
    Some(if lower.starts_with("ansi") {
        ColorMode::Ansi
    } else {
        ColorMode::Mono
    })
}

/// Parse `ascii_force_color`.
pub fn parse_ascii_force_color(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `output_mode`.
pub fn parse_output_mode(value: &str) -> RenderOutputMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("text") {
        RenderOutputMode::Text
    } else if lower.starts_with("kit") || lower.starts_with("graph") {
        RenderOutputMode::KittyHq
    } else {
        RenderOutputMode::Hybrid
    }
}

/// Parse `kitty_transport`, `transport`.
pub fn parse_kitty_transport(value: &str) -> KittyTransport {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("dir") {
        KittyTransport::Direct
    } else {
        KittyTransport::Shm
    }
}

/// Parse `kitty_compression`, `compression`.
pub fn parse_kitty_compression(value: &str) -> KittyCompression {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("z") {
        KittyCompression::Zlib
    } else {
        KittyCompression::None
    }
}

/// Parse `kitty_internal_res`, `internal_res`.
pub fn parse_kitty_internal_res(value: &str) -> KittyInternalResPreset {
    let lower = value.to_ascii_lowercase();
    if lower.contains("1280x720") || lower.contains("720") {
        KittyInternalResPreset::R1280x720
    } else if lower.contains("854x480") || lower.contains("480") {
        KittyInternalResPreset::R854x480
    } else {
        KittyInternalResPreset::R640x360
    }
}

/// Parse `kitty_pipeline`, `kitty_pipeline_mode`.
pub fn parse_kitty_pipeline_mode(value: &str) -> KittyPipelineMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("glyph") {
        KittyPipelineMode::GlyphCompat
    } else {
        KittyPipelineMode::RealPixel
    }
}

/// Parse `recover_strategy`.
pub fn parse_recover_strategy(value: &str) -> RecoverStrategy {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") || lower == "0" {
        RecoverStrategy::Off
    } else if lower.starts_with("soft") {
        RecoverStrategy::Soft
    } else {
        RecoverStrategy::Hard
    }
}

/// Parse `kitty_scale`.
pub fn parse_kitty_scale(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.5, 2.0))
}

/// Parse `hq_target_fps`, `kitty_target_fps`.
pub fn parse_hq_target_fps(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().map(|v| v.clamp(12, 120))
}

/// Parse `subject_exposure_only`.
pub fn parse_subject_exposure_only(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `subject_target_height`, `subject_target_height_ratio`.
pub fn parse_subject_target_height(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.20, 0.95))
}

/// Parse `subject_target_width`, `subject_target_width_ratio`.
pub fn parse_subject_target_width(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.10, 0.95))
}

/// Parse `quality_auto_distance`.
pub fn parse_quality_auto_distance(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `texture_mip_bias`.
pub fn parse_texture_mip_bias(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(-2.0, 4.0))
}

/// Parse `stage_as_sub_only`.
pub fn parse_stage_as_sub_only(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `stage_role`.
pub fn parse_stage_role(value: &str) -> StageRole {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") || lower == "0" {
        StageRole::Off
    } else {
        StageRole::Sub
    }
}

/// Parse `stage_quality`.
pub fn parse_stage_quality(value: &str) -> StageQuality {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("min") {
        StageQuality::Minimal
    } else if lower.starts_with("low") {
        StageQuality::Low
    } else if lower.starts_with("high") {
        StageQuality::High
    } else {
        StageQuality::Medium
    }
}

/// Parse `stage_luma_cap`.
pub fn parse_stage_luma_cap(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

/// Parse `recover_color`.
pub fn parse_recover_color(value: &str) -> bool {
    !value.to_ascii_lowercase().starts_with("off")
}

/// Parse `graphics_protocol`, `graphics`.
pub fn parse_graphics_protocol(value: &str) -> GraphicsProtocol {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("kit") {
        GraphicsProtocol::Kitty
    } else if lower.starts_with("iterm") {
        GraphicsProtocol::Iterm2
    } else if lower.starts_with("none") || lower == "0" || lower == "off" {
        GraphicsProtocol::None
    } else {
        GraphicsProtocol::Auto
    }
}

/// Parse `braille_profile`.
pub fn parse_braille_profile(value: &str) -> BrailleProfile {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("den") {
        BrailleProfile::Dense
    } else if lower.starts_with("nor") {
        BrailleProfile::Normal
    } else {
        BrailleProfile::Safe
    }
}

/// Parse `theme`, `theme_style`.
pub fn parse_theme_style(value: &str) -> ThemeStyle {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("neo") {
        ThemeStyle::Neon
    } else if lower.starts_with("hol") {
        ThemeStyle::Holo
    } else {
        ThemeStyle::Theater
    }
}

/// Parse `audio_reactive`.
pub fn parse_audio_reactive(value: &str) -> AudioReactiveMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") || lower == "0" {
        AudioReactiveMode::Off
    } else if lower.starts_with("high") {
        AudioReactiveMode::High
    } else {
        AudioReactiveMode::On
    }
}

/// Parse `cinematic_camera`.
pub fn parse_cinematic_camera(value: &str) -> CinematicCameraMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") || lower == "0" {
        CinematicCameraMode::Off
    } else if lower.starts_with("agg") {
        CinematicCameraMode::Aggressive
    } else {
        CinematicCameraMode::On
    }
}

/// Parse `reactive_gain`.
pub fn parse_reactive_gain(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

/// Parse `perf_profile`, `performance_profile`.
pub fn parse_perf_profile(value: &str) -> PerfProfile {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("cin") {
        PerfProfile::Cinematic
    } else if lower.starts_with("smo") {
        PerfProfile::Smooth
    } else {
        PerfProfile::Balanced
    }
}

/// Parse `detail_profile`.
pub fn parse_detail_profile(value: &str) -> DetailProfile {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("perf") {
        DetailProfile::Perf
    } else if lower.starts_with("ult") {
        DetailProfile::Ultra
    } else {
        DetailProfile::Balanced
    }
}

/// Parse `clarity_profile`.
pub fn parse_clarity_profile(value: &str) -> ClarityProfile {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("ext") {
        ClarityProfile::Extreme
    } else if lower.starts_with("bal") {
        ClarityProfile::Balanced
    } else {
        ClarityProfile::Sharp
    }
}

/// Parse `ansi_quantization`.
pub fn parse_ansi_quantization(value: &str) -> AnsiQuantization {
    let lower = value.to_ascii_lowercase();
    if lower == "off" || lower == "false" || lower == "0" {
        AnsiQuantization::Off
    } else {
        AnsiQuantization::Q216
    }
}

/// Parse `backend`, `render_backend`.
pub fn parse_backend(value: &str) -> RenderBackend {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("gpu") {
        if lower.contains("preview") {
            eprintln!("warning: backend=gpu-preview is deprecated. use backend=gpu.");
        }
        RenderBackend::Gpu
    } else {
        RenderBackend::Cpu
    }
}

/// Apply visual keys to config.
pub fn apply_visual(key: &str, value: &str, cfg: &mut GasciiConfig) {
    match key {
        "color_mode" => {
            if let Some(v) = parse_color_mode(value) {
                cfg.color_mode = Some(v);
            }
        }
        "ascii_force_color" => {
            if let Some(v) = parse_ascii_force_color(value) {
                cfg.ascii_force_color = v;
            }
        }
        "output_mode" => {
            cfg.output_mode = parse_output_mode(value);
        }
        "kitty_transport" | "transport" => {
            cfg.kitty_transport = parse_kitty_transport(value);
        }
        "kitty_compression" | "compression" => {
            cfg.kitty_compression = parse_kitty_compression(value);
        }
        "kitty_internal_res" | "internal_res" => {
            cfg.kitty_internal_res = parse_kitty_internal_res(value);
        }
        "kitty_pipeline" | "kitty_pipeline_mode" => {
            cfg.kitty_pipeline_mode = parse_kitty_pipeline_mode(value);
        }
        "recover_strategy" => {
            cfg.recover_strategy = parse_recover_strategy(value);
        }
        "kitty_scale" => {
            if let Some(v) = parse_kitty_scale(value) {
                cfg.kitty_scale = v;
            }
        }
        "hq_target_fps" | "kitty_target_fps" => {
            if let Some(v) = parse_hq_target_fps(value) {
                cfg.hq_target_fps = v;
            }
        }
        "subject_exposure_only" => {
            if let Some(v) = parse_subject_exposure_only(value) {
                cfg.subject_exposure_only = v;
            }
        }
        "subject_target_height" | "subject_target_height_ratio" => {
            if let Some(v) = parse_subject_target_height(value) {
                cfg.subject_target_height_ratio = v;
            }
        }
        "subject_target_width" | "subject_target_width_ratio" => {
            if let Some(v) = parse_subject_target_width(value) {
                cfg.subject_target_width_ratio = v;
            }
        }
        "quality_auto_distance" => {
            if let Some(v) = parse_quality_auto_distance(value) {
                cfg.quality_auto_distance = v;
            }
        }
        "texture_mip_bias" => {
            if let Some(v) = parse_texture_mip_bias(value) {
                cfg.texture_mip_bias = v;
            }
        }
        "stage_as_sub_only" => {
            if let Some(v) = parse_stage_as_sub_only(value) {
                cfg.stage_as_sub_only = v;
            }
        }
        "stage_role" => {
            cfg.stage_role = parse_stage_role(value);
        }
        "stage_quality" => {
            cfg.stage_quality = parse_stage_quality(value);
        }
        "stage_luma_cap" => {
            if let Some(v) = parse_stage_luma_cap(value) {
                cfg.stage_luma_cap = v;
            }
        }
        "recover_color" => {
            cfg.recover_color_auto = parse_recover_color(value);
        }
        "graphics_protocol" | "graphics" => {
            cfg.graphics_protocol = parse_graphics_protocol(value);
        }
        "braille_profile" => {
            cfg.braille_profile = parse_braille_profile(value);
        }
        "theme" | "theme_style" => {
            cfg.theme_style = parse_theme_style(value);
        }
        "audio_reactive" => {
            cfg.audio_reactive = parse_audio_reactive(value);
        }
        "cinematic_camera" => {
            cfg.cinematic_camera = parse_cinematic_camera(value);
        }
        "reactive_gain" => {
            if let Some(v) = parse_reactive_gain(value) {
                cfg.reactive_gain = v;
            }
        }
        "perf_profile" | "performance_profile" => {
            cfg.perf_profile = parse_perf_profile(value);
        }
        "detail_profile" => {
            cfg.detail_profile = parse_detail_profile(value);
        }
        "clarity_profile" => {
            cfg.clarity_profile = parse_clarity_profile(value);
        }
        "ansi_quantization" => {
            cfg.ansi_quantization = parse_ansi_quantization(value);
        }
        "backend" | "render_backend" => {
            cfg.backend = parse_backend(value);
        }
        _ => {}
    }
}
