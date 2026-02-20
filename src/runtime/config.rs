use std::{fs, path::Path};

use crate::scene::{CellAspectMode, ContrastProfile, SyncSpeedMode};

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
            "cell_aspect_mode = manual\ncell_aspect_trim = 1.15\ncontrast_profile = fixed\nsync_offset_ms = -120\nsync_speed_mode = realtime\n",
        )
        .expect("write config");

        let cfg = load_gascii_config(&path);
        assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Manual);
        assert_eq!(cfg.cell_aspect_trim, 1.15);
        assert_eq!(cfg.contrast_profile, ContrastProfile::Fixed);
        assert_eq!(cfg.sync_offset_ms, -120);
        assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::Realtime1x);
    }
}
