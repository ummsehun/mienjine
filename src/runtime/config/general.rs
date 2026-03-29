//! Parser for general settings: UI language, font presets, cell aspect, contrast.

use crate::runtime::config::types::{GasciiConfig, UiLanguage};
use crate::runtime::config_parse::parse_bool;
use crate::scene::{CellAspectMode, ContrastProfile};

/// Parse `ui_language`, `language`, `ui_lang`.
pub fn parse_ui_language(value: &str) -> UiLanguage {
    let v = value.to_ascii_lowercase();
    if v.starts_with("en") {
        UiLanguage::En
    } else {
        UiLanguage::Ko
    }
}

/// Parse `ghostty_font_steps`, `font_steps`, `font_preset_steps`.
pub fn parse_font_steps(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().map(|v| v.clamp(-30, 30))
}

/// Parse `ghostty_font_reset`, `font_reset`, `font_preset_enabled`.
pub fn parse_font_preset_enabled(value: &str) -> Option<bool> {
    parse_bool(value)
}

/// Parse `cell_aspect_mode`, `aspect_mode`.
pub fn parse_cell_aspect_mode(value: &str) -> CellAspectMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("man") {
        CellAspectMode::Manual
    } else {
        CellAspectMode::Auto
    }
}

/// Parse `cell_aspect_trim`, `aspect_trim`.
pub fn parse_cell_aspect_trim(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.70, 1.30))
}

/// Parse `contrast_profile`.
pub fn parse_contrast_profile(value: &str) -> ContrastProfile {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("fix") {
        ContrastProfile::Fixed
    } else {
        ContrastProfile::Adaptive
    }
}

/// Apply general keys to config.
pub fn apply_general(key: &str, value: &str, cfg: &mut GasciiConfig) {
    match key {
        "ui_language" | "language" | "ui_lang" => {
            cfg.ui_language = parse_ui_language(value);
        }
        "ghostty_font_steps" | "font_steps" => {
            if let Some(v) = parse_font_steps(value) {
                cfg.font_preset_steps = v;
            }
        }
        "ghostty_font_reset" | "font_reset" => {
            if let Some(v) = parse_font_preset_enabled(value) {
                cfg.font_preset_enabled = v;
            }
        }
        "font_preset_enabled" => {
            if let Some(v) = parse_font_preset_enabled(value) {
                cfg.font_preset_enabled = v;
            }
        }
        "font_preset_steps" => {
            if let Some(v) = parse_font_steps(value) {
                cfg.font_preset_steps = v;
            }
        }
        "cell_aspect_mode" | "aspect_mode" => {
            cfg.cell_aspect_mode = parse_cell_aspect_mode(value);
        }
        "cell_aspect_trim" | "aspect_trim" => {
            if let Some(v) = parse_cell_aspect_trim(value) {
                cfg.cell_aspect_trim = v;
            }
        }
        "contrast_profile" => {
            cfg.contrast_profile = parse_contrast_profile(value);
        }
        _ => {}
    }
}
