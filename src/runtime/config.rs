use std::{fs, path::Path};

use crate::scene::{
    AudioReactiveMode, BrailleProfile, CameraFocusMode, CellAspectMode, CenterLockMode,
    CinematicCameraMode, ColorMode, ContrastProfile, DetailProfile, PerfProfile, RenderBackend,
    SyncSpeedMode, TextureSamplingMode, ThemeStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLanguage {
    Ko,
    En,
}

#[derive(Debug, Clone, Copy)]
pub struct GasciiConfig {
    pub ui_language: UiLanguage,
    pub font_preset_steps: i32,
    pub font_preset_enabled: bool,
    pub color_mode: Option<ColorMode>,
    pub braille_profile: BrailleProfile,
    pub theme_style: ThemeStyle,
    pub audio_reactive: AudioReactiveMode,
    pub cinematic_camera: CinematicCameraMode,
    pub reactive_gain: f32,
    pub perf_profile: PerfProfile,
    pub detail_profile: DetailProfile,
    pub backend: RenderBackend,
    pub exposure_bias: f32,
    pub center_lock: bool,
    pub center_lock_mode: CenterLockMode,
    pub camera_focus: CameraFocusMode,
    pub material_color: bool,
    pub texture_sampling: TextureSamplingMode,
    pub braille_aspect_compensation: f32,
    pub stage_level: u8,
    pub stage_reactive: bool,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
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
            braille_profile: BrailleProfile::Safe,
            theme_style: ThemeStyle::Theater,
            audio_reactive: AudioReactiveMode::On,
            cinematic_camera: CinematicCameraMode::On,
            reactive_gain: 0.35,
            perf_profile: PerfProfile::Balanced,
            detail_profile: DetailProfile::Balanced,
            backend: RenderBackend::Cpu,
            exposure_bias: 0.0,
            center_lock: true,
            center_lock_mode: CenterLockMode::Root,
            camera_focus: CameraFocusMode::Auto,
            material_color: true,
            texture_sampling: TextureSamplingMode::Nearest,
            braille_aspect_compensation: 0.90,
            stage_level: 2,
            stage_reactive: true,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            contrast_profile: ContrastProfile::Adaptive,
            sync_offset_ms: 0,
            sync_speed_mode: SyncSpeedMode::AutoDurationFit,
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
            "braille_aspect_compensation" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    cfg.braille_aspect_compensation = parsed.clamp(0.70, 1.30);
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

fn parse_bool(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
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
        assert_eq!(cfg.braille_profile, BrailleProfile::Safe);
        assert_eq!(cfg.theme_style, ThemeStyle::Theater);
        assert_eq!(cfg.audio_reactive, AudioReactiveMode::On);
        assert_eq!(cfg.cinematic_camera, CinematicCameraMode::On);
        assert!((cfg.reactive_gain - 0.35).abs() < 1e-6);
        assert_eq!(cfg.perf_profile, PerfProfile::Balanced);
        assert_eq!(cfg.detail_profile, DetailProfile::Balanced);
        assert_eq!(cfg.backend, RenderBackend::Cpu);
        assert!((cfg.exposure_bias - 0.0).abs() < 1e-6);
        assert!(cfg.center_lock);
        assert_eq!(cfg.center_lock_mode, CenterLockMode::Root);
        assert_eq!(cfg.camera_focus, CameraFocusMode::Auto);
        assert!(cfg.material_color);
        assert_eq!(cfg.texture_sampling, TextureSamplingMode::Nearest);
        assert!((cfg.braille_aspect_compensation - 0.90).abs() < 1e-6);
        assert_eq!(cfg.stage_level, 2);
        assert!(cfg.stage_reactive);
        assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Auto);
        assert_eq!(cfg.cell_aspect_trim, 1.0);
        assert_eq!(cfg.contrast_profile, ContrastProfile::Adaptive);
        assert_eq!(cfg.sync_offset_ms, 0);
        assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::AutoDurationFit);
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
            "cell_aspect_mode = manual\ncell_aspect_trim = 1.15\ncontrast_profile = fixed\nsync_offset_ms = -120\nsync_speed_mode = realtime\ncolor_mode=ansi\nbraille_profile=normal\ntheme=holo\naudio_reactive=high\ncinematic_camera=aggressive\nreactive_gain=0.42\nperf_profile=smooth\ndetail_profile=ultra\nbackend=gpu-preview\nexposure_bias=0.18\ncenter_lock=false\ncenter_lock_mode=mixed\ncamera_focus=face\nmaterial_color=off\ntexture_sampling=bilinear\nbraille_aspect_compensation=1.12\nstage_level=4\nstage_reactive=off\n",
        )
        .expect("write config");

        let cfg = load_gascii_config(&path);
        assert_eq!(cfg.color_mode, Some(ColorMode::Ansi));
        assert_eq!(cfg.braille_profile, BrailleProfile::Normal);
        assert_eq!(cfg.theme_style, ThemeStyle::Holo);
        assert_eq!(cfg.audio_reactive, AudioReactiveMode::High);
        assert_eq!(cfg.cinematic_camera, CinematicCameraMode::Aggressive);
        assert!((cfg.reactive_gain - 0.42).abs() < 1e-6);
        assert_eq!(cfg.perf_profile, PerfProfile::Smooth);
        assert_eq!(cfg.detail_profile, DetailProfile::Ultra);
        assert_eq!(cfg.backend, RenderBackend::Gpu);
        assert!((cfg.exposure_bias - 0.18).abs() < 1e-6);
        assert!(!cfg.center_lock);
        assert_eq!(cfg.center_lock_mode, CenterLockMode::Mixed);
        assert_eq!(cfg.camera_focus, CameraFocusMode::Face);
        assert!(!cfg.material_color);
        assert_eq!(cfg.texture_sampling, TextureSamplingMode::Bilinear);
        assert!((cfg.braille_aspect_compensation - 1.12).abs() < 1e-6);
        assert_eq!(cfg.stage_level, 4);
        assert!(!cfg.stage_reactive);
        assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Manual);
        assert_eq!(cfg.cell_aspect_trim, 1.15);
        assert_eq!(cfg.contrast_profile, ContrastProfile::Fixed);
        assert_eq!(cfg.sync_offset_ms, -120);
        assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::Realtime1x);
    }
}
