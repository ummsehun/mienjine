//! Parser for camera settings: camera mode, movement, VMD, material, model lift, etc.

use std::path::PathBuf;

use crate::runtime::config::types::GasciiConfig;
use crate::runtime::config_parse::parse_bool;
use crate::scene::{
    CameraAlignPreset, CameraControlMode, CameraFocusMode, CameraMode, CenterLockMode,
    TextureSamplerMode, TextureSamplingMode, TextureVOrigin,
};

/// Parse `exposure_bias`.
pub fn parse_exposure_bias(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(-0.5, 0.8))
}

/// Parse `center_lock`.
pub fn parse_center_lock(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `center_lock_mode`.
pub fn parse_center_lock_mode(value: &str) -> CenterLockMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("mix") {
        CenterLockMode::Mixed
    } else {
        CenterLockMode::Root
    }
}

/// Parse `wasd_mode`, `camera_control_mode`.
pub fn parse_wasd_mode(value: &str) -> CameraControlMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("orb") {
        CameraControlMode::Orbit
    } else {
        CameraControlMode::FreeFly
    }
}

/// Parse `freefly_speed`.
pub fn parse_freefly_speed(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.1, 8.0))
}

/// Parse `camera_look_speed`.
pub fn parse_camera_look_speed(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.1, 8.0))
}

/// Parse `camera_dir`.
pub fn parse_camera_dir(value: &str) -> Option<PathBuf> {
    let raw = value.trim().trim_matches('"').trim_matches('\'');
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

/// Parse `camera_selection`, `camera`.
pub fn parse_camera_selection(value: &str) -> Option<String> {
    let raw = value.trim().trim_matches('"').trim_matches('\'');
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_owned())
    }
}

/// Parse `camera_mode`.
pub fn parse_camera_mode(value: &str) -> CameraMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") {
        CameraMode::Off
    } else if lower.starts_with("blend") {
        CameraMode::Blend
    } else {
        CameraMode::Vmd
    }
}

/// Parse `camera_align_preset`, `camera_preset`.
pub fn parse_camera_align_preset(value: &str) -> CameraAlignPreset {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("alt-a") || lower == "alta" {
        CameraAlignPreset::AltA
    } else if lower.starts_with("alt-b") || lower == "altb" {
        CameraAlignPreset::AltB
    } else {
        CameraAlignPreset::Std
    }
}

/// Parse `camera_unit_scale`.
pub fn parse_camera_unit_scale(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.01, 2.0))
}

/// Parse `camera_vmd_fps`.
pub fn parse_camera_vmd_fps(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(1.0, 240.0))
}

/// Parse `camera_vmd_path`.
pub fn parse_camera_vmd_path(value: &str) -> Option<Option<PathBuf>> {
    let raw = value.trim().trim_matches('"').trim_matches('\'');
    Some(if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    })
}

/// Parse `camera_focus`, `camera_focus_mode`.
pub fn parse_camera_focus(value: &str) -> CameraFocusMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("full") {
        CameraFocusMode::Full
    } else if lower.starts_with("upp") {
        CameraFocusMode::Upper
    } else if lower.starts_with("face") {
        CameraFocusMode::Face
    } else if lower.starts_with("hand") {
        CameraFocusMode::Hands
    } else {
        CameraFocusMode::Auto
    }
}

/// Parse `material_color`.
pub fn parse_material_color(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `texture_sampling`.
pub fn parse_texture_sampling(value: &str) -> TextureSamplingMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("bil") {
        TextureSamplingMode::Bilinear
    } else {
        TextureSamplingMode::Nearest
    }
}

/// Parse `texture_v_origin`.
pub fn parse_texture_v_origin(value: &str) -> TextureVOrigin {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("leg") {
        TextureVOrigin::Legacy
    } else {
        TextureVOrigin::Gltf
    }
}

/// Parse `texture_sampler`.
pub fn parse_texture_sampler(value: &str) -> TextureSamplerMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("over") {
        TextureSamplerMode::Override
    } else {
        TextureSamplerMode::Gltf
    }
}

/// Parse `braille_aspect_compensation`.
pub fn parse_braille_aspect_compensation(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.70, 1.30))
}

/// Parse `model_lift`.
pub fn parse_model_lift(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.02, 0.45))
}

/// Parse `edge_accent_strength`.
pub fn parse_edge_accent_strength(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.5))
}

/// Parse `bg_suppression`.
pub fn parse_bg_suppression(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

/// Parse `stage_level`.
pub fn parse_stage_level(value: &str) -> Option<u8> {
    value.parse::<u8>().ok().map(|v| v.min(4))
}

/// Parse `stage_reactive`.
pub fn parse_stage_reactive(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Apply camera keys to config.
pub fn apply_camera(key: &str, value: &str, cfg: &mut GasciiConfig) {
    match key {
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
            if let Some(v) = parse_exposure_bias(value) {
                cfg.exposure_bias = v;
            }
        }
        "center_lock" => {
            if let Some(v) = parse_center_lock(value) {
                cfg.center_lock = v;
            }
        }
        "center_lock_mode" => {
            cfg.center_lock_mode = parse_center_lock_mode(value);
        }
        "wasd_mode" | "camera_control_mode" => {
            cfg.wasd_mode = parse_wasd_mode(value);
        }
        "freefly_speed" => {
            if let Some(v) = parse_freefly_speed(value) {
                cfg.freefly_speed = v;
            }
        }
        "camera_look_speed" => {
            if let Some(v) = parse_camera_look_speed(value) {
                cfg.camera_look_speed = v;
            }
        }
        "camera_dir" => {
            if let Some(v) = parse_camera_dir(value) {
                cfg.camera_dir = v;
            }
        }
        "camera_selection" | "camera" => {
            if let Some(v) = parse_camera_selection(value) {
                cfg.camera_selection = v;
            }
        }
        "camera_mode" => {
            cfg.camera_mode = parse_camera_mode(value);
        }
        "camera_align_preset" | "camera_preset" => {
            cfg.camera_align_preset = parse_camera_align_preset(value);
        }
        "camera_unit_scale" => {
            if let Some(v) = parse_camera_unit_scale(value) {
                cfg.camera_unit_scale = v;
            }
        }
        "camera_vmd_fps" => {
            if let Some(v) = parse_camera_vmd_fps(value) {
                cfg.camera_vmd_fps = v;
            }
        }
        "camera_vmd_path" => {
            if let Some(Some(v)) = parse_camera_vmd_path(value) {
                cfg.camera_vmd_path = Some(v);
            } else if let Some(None) = parse_camera_vmd_path(value) {
                cfg.camera_vmd_path = None;
            }
        }
        "camera_focus" | "camera_focus_mode" => {
            cfg.camera_focus = parse_camera_focus(value);
        }
        "material_color" => {
            if let Some(v) = parse_material_color(value) {
                cfg.material_color = v;
            }
        }
        "texture_sampling" => {
            cfg.texture_sampling = parse_texture_sampling(value);
        }
        "texture_v_origin" => {
            cfg.texture_v_origin = parse_texture_v_origin(value);
        }
        "texture_sampler" => {
            cfg.texture_sampler = parse_texture_sampler(value);
        }
        "braille_aspect_compensation" => {
            if let Some(v) = parse_braille_aspect_compensation(value) {
                cfg.braille_aspect_compensation = v;
            }
        }
        "model_lift" => {
            if let Some(v) = parse_model_lift(value) {
                cfg.model_lift = v;
            }
        }
        "edge_accent_strength" => {
            if let Some(v) = parse_edge_accent_strength(value) {
                cfg.edge_accent_strength = v;
            }
        }
        "bg_suppression" => {
            if let Some(v) = parse_bg_suppression(value) {
                cfg.bg_suppression = v;
            }
        }
        "stage_level" => {
            if let Some(v) = parse_stage_level(value) {
                cfg.stage_level = v;
            }
        }
        "stage_reactive" => {
            if let Some(v) = parse_stage_reactive(value) {
                cfg.stage_reactive = v;
            }
        }
        _ => {}
    }
}
