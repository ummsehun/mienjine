use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::runtime::config_parse::parse_bool;

use crate::runtime::sync_profile::SyncProfileMode;
use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, KittyCompression,
    KittyInternalResPreset, KittyPipelineMode, KittyTransport, PerfProfile, RecoverStrategy,
    RenderBackend, RenderOutputMode, StageRole, SyncPolicy, SyncSpeedMode, TextureSamplerMode,
    TextureSamplingMode, TextureVOrigin, ThemeStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLanguage {
    Ko,
    En,
}

#[derive(Debug, Clone)]
pub struct GasciiConfig {
    pub ui_language: UiLanguage,
    pub font_preset_steps: i32,
    pub font_preset_enabled: bool,
    pub color_mode: Option<ColorMode>,
    pub ascii_force_color: bool,
    pub output_mode: RenderOutputMode,
    pub graphics_protocol: GraphicsProtocol,
    pub kitty_transport: KittyTransport,
    pub kitty_compression: KittyCompression,
    pub kitty_internal_res: KittyInternalResPreset,
    pub kitty_pipeline_mode: KittyPipelineMode,
    pub recover_strategy: RecoverStrategy,
    pub kitty_scale: f32,
    pub hq_target_fps: u32,
    pub subject_exposure_only: bool,
    pub subject_target_height_ratio: f32,
    pub subject_target_width_ratio: f32,
    pub quality_auto_distance: bool,
    pub texture_mip_bias: f32,
    pub stage_as_sub_only: bool,
    pub stage_role: StageRole,
    pub stage_luma_cap: f32,
    pub recover_color_auto: bool,
    pub braille_profile: BrailleProfile,
    pub theme_style: ThemeStyle,
    pub audio_reactive: AudioReactiveMode,
    pub cinematic_camera: CinematicCameraMode,
    pub reactive_gain: f32,
    pub perf_profile: PerfProfile,
    pub detail_profile: DetailProfile,
    pub clarity_profile: ClarityProfile,
    pub ansi_quantization: AnsiQuantization,
    pub backend: RenderBackend,
    pub stage_dir: PathBuf,
    pub stage_selection: String,
    pub exposure_bias: f32,
    pub center_lock: bool,
    pub center_lock_mode: CenterLockMode,
    pub wasd_mode: CameraControlMode,
    pub freefly_speed: f32,
    pub camera_look_speed: f32,
    pub camera_dir: PathBuf,
    pub camera_selection: String,
    pub camera_mode: CameraMode,
    pub camera_align_preset: CameraAlignPreset,
    pub camera_unit_scale: f32,
    pub camera_vmd_fps: f32,
    pub camera_vmd_path: Option<PathBuf>,
    pub camera_focus: CameraFocusMode,
    pub material_color: bool,
    pub texture_sampling: TextureSamplingMode,
    pub texture_v_origin: TextureVOrigin,
    pub texture_sampler: TextureSamplerMode,
    pub braille_aspect_compensation: f32,
    pub model_lift: f32,
    pub edge_accent_strength: f32,
    pub bg_suppression: f32,
    pub stage_level: u8,
    pub stage_reactive: bool,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
    pub sync_policy: SyncPolicy,
    pub sync_hard_snap_ms: u32,
    pub sync_kp: f32,
    pub sync_profile_dir: PathBuf,
    pub sync_profile_mode: SyncProfileMode,
    pub upscale_factor: u32,
    pub upscale_sharpen: f32,
    pub triangle_stride: usize,
    pub min_triangle_area_px2: f32,
}

impl Default for GasciiConfig {
    fn default() -> Self {
        Self {
            ui_language: UiLanguage::Ko,
            font_preset_steps: 0,
            font_preset_enabled: false,
            color_mode: None,
            ascii_force_color: true,
            output_mode: RenderOutputMode::Text,
            graphics_protocol: GraphicsProtocol::Auto,
            kitty_transport: KittyTransport::Shm,
            kitty_compression: KittyCompression::None,
            kitty_internal_res: KittyInternalResPreset::R640x360,
            kitty_pipeline_mode: KittyPipelineMode::RealPixel,
            recover_strategy: RecoverStrategy::Hard,
            kitty_scale: 1.0,
            hq_target_fps: 24,
            subject_exposure_only: true,
            subject_target_height_ratio: 0.66,
            subject_target_width_ratio: 0.42,
            quality_auto_distance: true,
            texture_mip_bias: 0.0,
            stage_as_sub_only: true,
            stage_role: StageRole::Sub,
            stage_luma_cap: 0.35,
            recover_color_auto: true,
            braille_profile: BrailleProfile::Safe,
            theme_style: ThemeStyle::Theater,
            audio_reactive: AudioReactiveMode::On,
            cinematic_camera: CinematicCameraMode::On,
            reactive_gain: 0.35,
            perf_profile: PerfProfile::Balanced,
            detail_profile: DetailProfile::Balanced,
            clarity_profile: ClarityProfile::Sharp,
            ansi_quantization: AnsiQuantization::Q216,
            backend: RenderBackend::Cpu,
            stage_dir: PathBuf::from("assets/stage"),
            stage_selection: "auto".to_owned(),
            exposure_bias: 0.0,
            center_lock: true,
            center_lock_mode: CenterLockMode::Root,
            wasd_mode: CameraControlMode::FreeFly,
            freefly_speed: 1.0,
            camera_look_speed: 1.0,
            camera_dir: PathBuf::from("assets/camera"),
            camera_selection: "none".to_owned(),
            camera_mode: CameraMode::Off,
            camera_align_preset: CameraAlignPreset::Std,
            camera_unit_scale: 0.08,
            camera_vmd_fps: 30.0,
            camera_vmd_path: None,
            camera_focus: CameraFocusMode::Auto,
            material_color: true,
            texture_sampling: TextureSamplingMode::Nearest,
            texture_v_origin: TextureVOrigin::Gltf,
            texture_sampler: TextureSamplerMode::Gltf,
            braille_aspect_compensation: 1.00,
            model_lift: 0.12,
            edge_accent_strength: 0.32,
            bg_suppression: 0.35,
            stage_level: 2,
            stage_reactive: true,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            contrast_profile: ContrastProfile::Adaptive,
            sync_offset_ms: 0,
            sync_speed_mode: SyncSpeedMode::AutoDurationFit,
            sync_policy: SyncPolicy::Continuous,
            sync_hard_snap_ms: 120,
            sync_kp: 0.15,
            sync_profile_dir: PathBuf::from("assets/sync"),
            sync_profile_mode: SyncProfileMode::Auto,
            upscale_factor: 2,
            upscale_sharpen: 0.20,
            triangle_stride: 1,
            min_triangle_area_px2: 0.0,
        }
    }
}

pub fn load_gascii_config(path: &Path) -> GasciiConfig {
    let Ok(content) = fs::read_to_string(path) else {
        return GasciiConfig::default();
    };
    let mut cfg = GasciiConfig::default();
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .replace(' ', "_");
        let value = raw_value.trim();
        match key.as_str() {
            "ui_language" | "language" | "ui_lang" => {
                let v = value.to_ascii_lowercase();
                cfg.ui_language = if v.starts_with("en") {
                    UiLanguage::En
                } else {
                    UiLanguage::Ko
                };
            }
            "ghostty_font_steps" | "font_steps" => {
                if let Ok(parsed) = value.parse::<i32>() {
                    cfg.font_preset_steps = parsed.clamp(-30, 30);
                }
            }
            "ghostty_font_reset" | "font_reset" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.font_preset_enabled = parsed;
                }
            }
            "font_preset_enabled" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.font_preset_enabled = parsed;
                }
            }
            "font_preset_steps" => {
                if let Ok(parsed) = value.parse::<i32>() {
                    cfg.font_preset_steps = parsed.clamp(-30, 30);
                }
            }
            "cell_aspect_mode" | "aspect_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.cell_aspect_mode = if lower.starts_with("man") {
                    CellAspectMode::Manual
                } else {
                    CellAspectMode::Auto
                };
            }
            "cell_aspect_trim" | "aspect_trim" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.cell_aspect_trim = parsed.clamp(0.70, 1.30);
                }
            }
            "contrast_profile" => {
                let lower = value.to_ascii_lowercase();
                cfg.contrast_profile = if lower.starts_with("fix") {
                    ContrastProfile::Fixed
                } else {
                    ContrastProfile::Adaptive
                };
            }
            "sync_offset_ms" => {
                if let Ok(parsed) = value.parse::<i32>() {
                    cfg.sync_offset_ms = parsed.clamp(-5000, 5000);
                }
            }
            "sync_speed_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.sync_speed_mode = if lower.starts_with("real") || lower == "1x" {
                    SyncSpeedMode::Realtime1x
                } else {
                    SyncSpeedMode::AutoDurationFit
                };
            }
            "sync_policy" => {
                let lower = value.to_ascii_lowercase();
                cfg.sync_policy = if lower.starts_with("fix") {
                    SyncPolicy::Fixed
                } else if lower.starts_with("man") {
                    SyncPolicy::Manual
                } else {
                    SyncPolicy::Continuous
                };
            }
            "sync_hard_snap_ms" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    cfg.sync_hard_snap_ms = parsed.clamp(10, 2000);
                }
            }
            "sync_kp" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.sync_kp = parsed.clamp(0.01, 1.0);
                }
            }
            "sync_profile_dir" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    cfg.sync_profile_dir = PathBuf::from(raw);
                }
            }
            "sync_profile_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.sync_profile_mode = if lower.starts_with("off") || lower == "0" {
                    SyncProfileMode::Off
                } else if lower.starts_with("wri") {
                    SyncProfileMode::Write
                } else {
                    SyncProfileMode::Auto
                };
            }
            "upscale_factor" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    cfg.upscale_factor = match parsed {
                        1 | 2 | 4 => parsed,
                        _ => 2,
                    };
                }
            }
            "upscale_sharpen" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.upscale_sharpen = parsed.clamp(0.0, 2.0);
                }
            }
            "triangle_stride" | "tri_stride" => {
                if let Ok(parsed) = value.parse::<usize>() {
                    cfg.triangle_stride = parsed.clamp(1, 16);
                }
            }
            "min_triangle_area_px2" | "tiny_triangle_area_px2" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.min_triangle_area_px2 = parsed.clamp(0.0, 16.0);
                }
            }
            "color_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.color_mode = Some(if lower.starts_with("ansi") {
                    ColorMode::Ansi
                } else {
                    ColorMode::Mono
                });
            }
            "ascii_force_color" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.ascii_force_color = parsed;
                }
            }
            "output_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.output_mode = if lower.starts_with("text") {
                    RenderOutputMode::Text
                } else if lower.starts_with("kit") || lower.starts_with("graph") {
                    RenderOutputMode::KittyHq
                } else {
                    RenderOutputMode::Hybrid
                };
            }
            "kitty_transport" | "transport" => {
                let lower = value.to_ascii_lowercase();
                cfg.kitty_transport = if lower.starts_with("dir") {
                    KittyTransport::Direct
                } else {
                    KittyTransport::Shm
                };
            }
            "kitty_compression" | "compression" => {
                let lower = value.to_ascii_lowercase();
                cfg.kitty_compression = if lower.starts_with("z") {
                    KittyCompression::Zlib
                } else {
                    KittyCompression::None
                };
            }
            "kitty_internal_res" | "internal_res" => {
                let lower = value.to_ascii_lowercase();
                cfg.kitty_internal_res = if lower.contains("1280x720") || lower.contains("720") {
                    KittyInternalResPreset::R1280x720
                } else if lower.contains("854x480") || lower.contains("480") {
                    KittyInternalResPreset::R854x480
                } else {
                    KittyInternalResPreset::R640x360
                };
            }
            "kitty_pipeline" | "kitty_pipeline_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.kitty_pipeline_mode = if lower.starts_with("glyph") {
                    KittyPipelineMode::GlyphCompat
                } else {
                    KittyPipelineMode::RealPixel
                };
            }
            "recover_strategy" => {
                let lower = value.to_ascii_lowercase();
                cfg.recover_strategy = if lower.starts_with("off") || lower == "0" {
                    RecoverStrategy::Off
                } else if lower.starts_with("soft") {
                    RecoverStrategy::Soft
                } else {
                    RecoverStrategy::Hard
                };
            }
            "kitty_scale" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.kitty_scale = parsed.clamp(0.5, 2.0);
                }
            }
            "hq_target_fps" | "kitty_target_fps" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    cfg.hq_target_fps = parsed.clamp(12, 120);
                }
            }
            "subject_exposure_only" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.subject_exposure_only = parsed;
                }
            }
            "subject_target_height" | "subject_target_height_ratio" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.subject_target_height_ratio = parsed.clamp(0.20, 0.95);
                }
            }
            "subject_target_width" | "subject_target_width_ratio" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.subject_target_width_ratio = parsed.clamp(0.10, 0.95);
                }
            }
            "quality_auto_distance" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.quality_auto_distance = parsed;
                }
            }
            "texture_mip_bias" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.texture_mip_bias = parsed.clamp(-2.0, 4.0);
                }
            }
            "stage_as_sub_only" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.stage_as_sub_only = parsed;
                }
            }
            "stage_role" => {
                let lower = value.to_ascii_lowercase();
                cfg.stage_role = if lower.starts_with("off") || lower == "0" {
                    StageRole::Off
                } else {
                    StageRole::Sub
                };
            }
            "stage_luma_cap" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.stage_luma_cap = parsed.clamp(0.0, 1.0);
                }
            }
            "recover_color" => {
                let lower = value.to_ascii_lowercase();
                cfg.recover_color_auto = !lower.starts_with("off");
            }
            "graphics_protocol" | "graphics" => {
                let lower = value.to_ascii_lowercase();
                cfg.graphics_protocol = if lower.starts_with("kit") {
                    GraphicsProtocol::Kitty
                } else if lower.starts_with("iterm") {
                    GraphicsProtocol::Iterm2
                } else if lower.starts_with("none") || lower == "0" || lower == "off" {
                    GraphicsProtocol::None
                } else {
                    GraphicsProtocol::Auto
                };
            }
            "braille_profile" => {
                let lower = value.to_ascii_lowercase();
                cfg.braille_profile = if lower.starts_with("den") {
                    BrailleProfile::Dense
                } else if lower.starts_with("nor") {
                    BrailleProfile::Normal
                } else {
                    BrailleProfile::Safe
                };
            }
            "theme" | "theme_style" => {
                let lower = value.to_ascii_lowercase();
                cfg.theme_style = if lower.starts_with("neo") {
                    ThemeStyle::Neon
                } else if lower.starts_with("hol") {
                    ThemeStyle::Holo
                } else {
                    ThemeStyle::Theater
                };
            }
            "audio_reactive" => {
                let lower = value.to_ascii_lowercase();
                cfg.audio_reactive = if lower.starts_with("off") || lower == "0" {
                    AudioReactiveMode::Off
                } else if lower.starts_with("high") {
                    AudioReactiveMode::High
                } else {
                    AudioReactiveMode::On
                };
            }
            "cinematic_camera" => {
                let lower = value.to_ascii_lowercase();
                cfg.cinematic_camera = if lower.starts_with("off") || lower == "0" {
                    CinematicCameraMode::Off
                } else if lower.starts_with("agg") {
                    CinematicCameraMode::Aggressive
                } else {
                    CinematicCameraMode::On
                };
            }
            "reactive_gain" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.reactive_gain = parsed.clamp(0.0, 1.0);
                }
            }
            "perf_profile" | "performance_profile" => {
                let lower = value.to_ascii_lowercase();
                cfg.perf_profile = if lower.starts_with("cin") {
                    PerfProfile::Cinematic
                } else if lower.starts_with("smo") {
                    PerfProfile::Smooth
                } else {
                    PerfProfile::Balanced
                };
            }
            "detail_profile" => {
                let lower = value.to_ascii_lowercase();
                cfg.detail_profile = if lower.starts_with("perf") {
                    DetailProfile::Perf
                } else if lower.starts_with("ult") {
                    DetailProfile::Ultra
                } else {
                    DetailProfile::Balanced
                };
            }
            "clarity_profile" => {
                let lower = value.to_ascii_lowercase();
                cfg.clarity_profile = if lower.starts_with("ext") {
                    ClarityProfile::Extreme
                } else if lower.starts_with("bal") {
                    ClarityProfile::Balanced
                } else {
                    ClarityProfile::Sharp
                };
            }
            "ansi_quantization" => {
                let lower = value.to_ascii_lowercase();
                cfg.ansi_quantization = if lower == "off" || lower == "false" || lower == "0" {
                    AnsiQuantization::Off
                } else {
                    AnsiQuantization::Q216
                };
            }
            "backend" | "render_backend" => {
                let lower = value.to_ascii_lowercase();
                cfg.backend = if lower.starts_with("gpu") {
                    if lower.contains("preview") {
                        eprintln!("warning: backend=gpu-preview is deprecated. use backend=gpu.");
                    }
                    RenderBackend::Gpu
                } else {
                    RenderBackend::Cpu
                };
            }
            "stage_dir" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    cfg.stage_dir = PathBuf::from(raw);
                }
            }
            "stage_selection" | "stage" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    cfg.stage_selection = raw.to_owned();
                }
            }
            "exposure_bias" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.exposure_bias = parsed.clamp(-0.5, 0.8);
                }
            }
            "center_lock" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.center_lock = parsed;
                }
            }
            "center_lock_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.center_lock_mode = if lower.starts_with("mix") {
                    CenterLockMode::Mixed
                } else {
                    CenterLockMode::Root
                };
            }
            "wasd_mode" | "camera_control_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.wasd_mode = if lower.starts_with("orb") {
                    CameraControlMode::Orbit
                } else {
                    CameraControlMode::FreeFly
                };
            }
            "freefly_speed" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.freefly_speed = parsed.clamp(0.1, 8.0);
                }
            }
            "camera_look_speed" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.camera_look_speed = parsed.clamp(0.1, 8.0);
                }
            }
            "camera_dir" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    cfg.camera_dir = PathBuf::from(raw);
                }
            }
            "camera_selection" | "camera" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    cfg.camera_selection = raw.to_owned();
                }
            }
            "camera_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.camera_mode = if lower.starts_with("off") {
                    CameraMode::Off
                } else if lower.starts_with("blend") {
                    CameraMode::Blend
                } else {
                    CameraMode::Vmd
                };
            }
            "camera_align_preset" | "camera_preset" => {
                let lower = value.to_ascii_lowercase();
                cfg.camera_align_preset = if lower.starts_with("alt-a") || lower == "alta" {
                    CameraAlignPreset::AltA
                } else if lower.starts_with("alt-b") || lower == "altb" {
                    CameraAlignPreset::AltB
                } else {
                    CameraAlignPreset::Std
                };
            }
            "camera_unit_scale" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.camera_unit_scale = parsed.clamp(0.01, 2.0);
                }
            }
            "camera_vmd_fps" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.camera_vmd_fps = parsed.clamp(1.0, 240.0);
                }
            }
            "camera_vmd_path" => {
                let raw = value.trim().trim_matches('"').trim_matches('\'');
                if raw.is_empty() {
                    cfg.camera_vmd_path = None;
                } else {
                    cfg.camera_vmd_path = Some(PathBuf::from(raw));
                }
            }
            "camera_focus" | "camera_focus_mode" => {
                let lower = value.to_ascii_lowercase();
                cfg.camera_focus = if lower.starts_with("full") {
                    CameraFocusMode::Full
                } else if lower.starts_with("upp") {
                    CameraFocusMode::Upper
                } else if lower.starts_with("face") {
                    CameraFocusMode::Face
                } else if lower.starts_with("hand") {
                    CameraFocusMode::Hands
                } else {
                    CameraFocusMode::Auto
                };
            }
            "material_color" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.material_color = parsed;
                }
            }
            "texture_sampling" => {
                let lower = value.to_ascii_lowercase();
                cfg.texture_sampling = if lower.starts_with("bil") {
                    TextureSamplingMode::Bilinear
                } else {
                    TextureSamplingMode::Nearest
                };
            }
            "texture_v_origin" => {
                let lower = value.to_ascii_lowercase();
                cfg.texture_v_origin = if lower.starts_with("leg") {
                    TextureVOrigin::Legacy
                } else {
                    TextureVOrigin::Gltf
                };
            }
            "texture_sampler" => {
                let lower = value.to_ascii_lowercase();
                cfg.texture_sampler = if lower.starts_with("over") {
                    TextureSamplerMode::Override
                } else {
                    TextureSamplerMode::Gltf
                };
            }
            "braille_aspect_compensation" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.braille_aspect_compensation = parsed.clamp(0.70, 1.30);
                }
            }
            "model_lift" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.model_lift = parsed.clamp(0.02, 0.45);
                }
            }
            "edge_accent_strength" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.edge_accent_strength = parsed.clamp(0.0, 1.5);
                }
            }
            "bg_suppression" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.bg_suppression = parsed.clamp(0.0, 1.0);
                }
            }
            "stage_level" => {
                if let Ok(parsed) = value.parse::<u8>() {
                    cfg.stage_level = parsed.min(4);
                }
            }
            "stage_reactive" => {
                if let Some(parsed) = parse_bool(value) {
                    cfg.stage_reactive = parsed;
                }
            }
            _ => {}
        }
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = GasciiConfig::default();
        assert_eq!(cfg.ui_language, UiLanguage::Ko);
        assert_eq!(cfg.font_preset_steps, 0);
        assert!(!cfg.font_preset_enabled);
        assert_eq!(cfg.color_mode, None);
        assert!(cfg.ascii_force_color);
        assert_eq!(cfg.output_mode, RenderOutputMode::Text);
        assert_eq!(cfg.graphics_protocol, GraphicsProtocol::Auto);
        assert_eq!(cfg.kitty_transport, KittyTransport::Shm);
        assert_eq!(cfg.kitty_compression, KittyCompression::None);
        assert_eq!(cfg.kitty_internal_res, KittyInternalResPreset::R640x360);
        assert!((cfg.kitty_scale - 1.0).abs() < 1e-6);
        assert_eq!(cfg.hq_target_fps, 24);
        assert!(cfg.subject_exposure_only);
        assert_eq!(cfg.stage_role, StageRole::Sub);
        assert!((cfg.stage_luma_cap - 0.35).abs() < 1e-6);
        assert!(cfg.recover_color_auto);
        assert_eq!(cfg.braille_profile, BrailleProfile::Safe);
        assert_eq!(cfg.theme_style, ThemeStyle::Theater);
        assert_eq!(cfg.audio_reactive, AudioReactiveMode::On);
        assert_eq!(cfg.cinematic_camera, CinematicCameraMode::On);
        assert!((cfg.reactive_gain - 0.35).abs() < 1e-6);
        assert_eq!(cfg.perf_profile, PerfProfile::Balanced);
        assert_eq!(cfg.detail_profile, DetailProfile::Balanced);
        assert_eq!(cfg.clarity_profile, ClarityProfile::Sharp);
        assert_eq!(cfg.ansi_quantization, AnsiQuantization::Q216);
        assert_eq!(cfg.backend, RenderBackend::Cpu);
        assert_eq!(cfg.stage_dir, PathBuf::from("assets/stage"));
        assert_eq!(cfg.stage_selection, "auto");
        assert!((cfg.exposure_bias - 0.0).abs() < 1e-6);
        assert!(cfg.center_lock);
        assert_eq!(cfg.center_lock_mode, CenterLockMode::Root);
        assert_eq!(cfg.wasd_mode, CameraControlMode::FreeFly);
        assert!((cfg.freefly_speed - 1.0).abs() < 1e-6);
        assert!((cfg.camera_look_speed - 1.0).abs() < 1e-6);
        assert_eq!(cfg.camera_dir, PathBuf::from("assets/camera"));
        assert_eq!(cfg.camera_selection, "none");
        assert_eq!(cfg.camera_mode, CameraMode::Off);
        assert_eq!(cfg.camera_align_preset, CameraAlignPreset::Std);
        assert!((cfg.camera_unit_scale - 0.08).abs() < 1e-6);
        assert!((cfg.camera_vmd_fps - 30.0).abs() < 1e-6);
        assert_eq!(cfg.camera_vmd_path, None);
        assert_eq!(cfg.camera_focus, CameraFocusMode::Auto);
        assert!(cfg.material_color);
        assert_eq!(cfg.texture_sampling, TextureSamplingMode::Nearest);
        assert_eq!(cfg.texture_v_origin, TextureVOrigin::Gltf);
        assert_eq!(cfg.texture_sampler, TextureSamplerMode::Gltf);
        assert!((cfg.braille_aspect_compensation - 1.00).abs() < 1e-6);
        assert!((cfg.model_lift - 0.12).abs() < 1e-6);
        assert!((cfg.edge_accent_strength - 0.32).abs() < 1e-6);
        assert!((cfg.bg_suppression - 0.35).abs() < 1e-6);
        assert_eq!(cfg.stage_level, 2);
        assert!(cfg.stage_reactive);
        assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Auto);
        assert_eq!(cfg.cell_aspect_trim, 1.0);
        assert_eq!(cfg.contrast_profile, ContrastProfile::Adaptive);
        assert_eq!(cfg.sync_offset_ms, 0);
        assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::AutoDurationFit);
        assert_eq!(cfg.sync_policy, SyncPolicy::Continuous);
        assert_eq!(cfg.sync_hard_snap_ms, 120);
        assert!((cfg.sync_kp - 0.15).abs() < 1e-6);
        assert_eq!(cfg.sync_profile_dir, PathBuf::from("assets/sync"));
        assert_eq!(cfg.sync_profile_mode, SyncProfileMode::Auto);
        assert_eq!(cfg.upscale_factor, 2);
        assert!((cfg.upscale_sharpen - 0.20).abs() < 1e-6);
        assert_eq!(cfg.triangle_stride, 1);
        assert_eq!(cfg.min_triangle_area_px2, 0.0);
    }

    #[test]
    fn parse_bool_variants() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("off"), Some(false));
        assert_eq!(parse_bool("??"), None);
    }

    #[test]
    fn legacy_font_keys_remain_compatible() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Gascii.config");
        fs::write(
            &path,
            "ghostty_font_reset = true\nghostty_font_steps = 3\nui_language = en\n",
        )
        .expect("write config");

        let cfg = load_gascii_config(&path);
        assert_eq!(cfg.ui_language, UiLanguage::En);
        assert!(cfg.font_preset_enabled);
        assert_eq!(cfg.font_preset_steps, 3);
    }

    #[test]
    fn normalized_font_keys_parse_correctly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Gascii.config");
        fs::write(
            &path,
            "font_preset_enabled = true\nfont_preset_steps = -2\ntriangle_stride = 4\n",
        )
        .expect("write config");

        let cfg = load_gascii_config(&path);
        assert!(cfg.font_preset_enabled);
        assert_eq!(cfg.font_preset_steps, -2);
        assert_eq!(cfg.triangle_stride, 4);
    }

    #[test]
    fn parse_new_visual_and_sync_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Gascii.config");
        fs::write(
            &path,
            "cell_aspect_mode = manual\ncell_aspect_trim = 1.15\ncontrast_profile = fixed\nsync_offset_ms = -120\nsync_speed_mode = realtime\nsync_policy = fixed\nsync_hard_snap_ms = 160\nsync_kp = 0.2\nsync_profile_dir = assets/sync/custom\nsync_profile_mode = write\ncolor_mode=ansi\nascii_force_color=false\noutput_mode=kitty-hq\nkitty_transport=direct\nkitty_compression=zlib\nkitty_internal_res=1280x720\nkitty_scale=1.25\nhq_target_fps=30\nsubject_exposure_only=off\nstage_role=off\nstage_luma_cap=0.55\nrecover_color=off\ngraphics_protocol=kitty\nupscale_factor=4\nupscale_sharpen=0.6\nbraille_profile=normal\ntheme=holo\naudio_reactive=high\ncinematic_camera=aggressive\nreactive_gain=0.42\nperf_profile=smooth\ndetail_profile=ultra\nclarity_profile=extreme\nansi_quantization=off\nbackend=gpu-preview\nstage_dir=assets/stage\nstage_selection=world is mine\nexposure_bias=0.18\ncenter_lock=false\ncenter_lock_mode=mixed\nwasd_mode=orbit\nfreefly_speed=2.4\ncamera_look_speed=1.8\ncamera_dir=assets/camera\ncamera_selection=none\ncamera_mode=blend\ncamera_align_preset=alt-b\ncamera_unit_scale=0.12\ncamera_vmd_fps=60\ncamera_vmd_path=assets/camera/world_is_mine.vmd\ncamera_focus=face\nmaterial_color=off\ntexture_sampling=bilinear\ntexture_v_origin=legacy\ntexture_sampler=override\nbraille_aspect_compensation=1.12\nmodel_lift=0.2\nedge_accent_strength=0.5\nbg_suppression=0.42\nstage_level=4\nstage_reactive=off\n",
        )
        .expect("write config");

        let cfg = load_gascii_config(&path);
        assert_eq!(cfg.color_mode, Some(ColorMode::Ansi));
        assert!(!cfg.ascii_force_color);
        assert_eq!(cfg.output_mode, RenderOutputMode::KittyHq);
        assert_eq!(cfg.kitty_transport, KittyTransport::Direct);
        assert_eq!(cfg.kitty_compression, KittyCompression::Zlib);
        assert_eq!(cfg.kitty_internal_res, KittyInternalResPreset::R1280x720);
        assert!((cfg.kitty_scale - 1.25).abs() < 1e-6);
        assert_eq!(cfg.hq_target_fps, 30);
        assert!(!cfg.subject_exposure_only);
        assert_eq!(cfg.stage_role, StageRole::Off);
        assert!((cfg.stage_luma_cap - 0.55).abs() < 1e-6);
        assert!(!cfg.recover_color_auto);
        assert_eq!(cfg.graphics_protocol, GraphicsProtocol::Kitty);
        assert_eq!(cfg.upscale_factor, 4);
        assert!((cfg.upscale_sharpen - 0.6).abs() < 1e-6);
        assert_eq!(cfg.braille_profile, BrailleProfile::Normal);
        assert_eq!(cfg.theme_style, ThemeStyle::Holo);
        assert_eq!(cfg.audio_reactive, AudioReactiveMode::High);
        assert_eq!(cfg.cinematic_camera, CinematicCameraMode::Aggressive);
        assert!((cfg.reactive_gain - 0.42).abs() < 1e-6);
        assert_eq!(cfg.perf_profile, PerfProfile::Smooth);
        assert_eq!(cfg.detail_profile, DetailProfile::Ultra);
        assert_eq!(cfg.clarity_profile, ClarityProfile::Extreme);
        assert_eq!(cfg.ansi_quantization, AnsiQuantization::Off);
        assert_eq!(cfg.backend, RenderBackend::Gpu);
        assert_eq!(cfg.stage_dir, PathBuf::from("assets/stage"));
        assert_eq!(cfg.stage_selection, "world is mine");
        assert!((cfg.exposure_bias - 0.18).abs() < 1e-6);
        assert!(!cfg.center_lock);
        assert_eq!(cfg.center_lock_mode, CenterLockMode::Mixed);
        assert_eq!(cfg.wasd_mode, CameraControlMode::Orbit);
        assert!((cfg.freefly_speed - 2.4).abs() < 1e-6);
        assert!((cfg.camera_look_speed - 1.8).abs() < 1e-6);
        assert_eq!(cfg.camera_dir, PathBuf::from("assets/camera"));
        assert_eq!(cfg.camera_selection, "none");
        assert_eq!(cfg.camera_mode, CameraMode::Blend);
        assert_eq!(cfg.camera_align_preset, CameraAlignPreset::AltB);
        assert!((cfg.camera_unit_scale - 0.12).abs() < 1e-6);
        assert!((cfg.camera_vmd_fps - 60.0).abs() < 1e-6);
        assert_eq!(
            cfg.camera_vmd_path.as_deref(),
            Some(Path::new("assets/camera/world_is_mine.vmd"))
        );
        assert_eq!(cfg.camera_focus, CameraFocusMode::Face);
        assert!(!cfg.material_color);
        assert_eq!(cfg.texture_sampling, TextureSamplingMode::Bilinear);
        assert_eq!(cfg.texture_v_origin, TextureVOrigin::Legacy);
        assert_eq!(cfg.texture_sampler, TextureSamplerMode::Override);
        assert!((cfg.braille_aspect_compensation - 1.12).abs() < 1e-6);
        assert!((cfg.model_lift - 0.2).abs() < 1e-6);
        assert!((cfg.edge_accent_strength - 0.5).abs() < 1e-6);
        assert!((cfg.bg_suppression - 0.42).abs() < 1e-6);
        assert_eq!(cfg.stage_level, 4);
        assert!(!cfg.stage_reactive);
        assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Manual);
        assert_eq!(cfg.cell_aspect_trim, 1.15);
        assert_eq!(cfg.contrast_profile, ContrastProfile::Fixed);
        assert_eq!(cfg.sync_offset_ms, -120);
        assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::Realtime1x);
        assert_eq!(cfg.sync_policy, SyncPolicy::Fixed);
        assert_eq!(cfg.sync_hard_snap_ms, 160);
        assert!((cfg.sync_kp - 0.2).abs() < 1e-6);
        assert_eq!(cfg.sync_profile_dir, PathBuf::from("assets/sync/custom"));
        assert_eq!(cfg.sync_profile_mode, SyncProfileMode::Write);
    }
}
