use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::helpers::{
    default_ansi_quantization, default_audio_reactive, default_backend,
    default_braille_aspect_compensation, default_braille_profile, default_branch,
    default_camera_align_preset, default_camera_focus, default_camera_mode,
    default_camera_unit_scale, default_cell_aspect_mode, default_cell_aspect_trim,
    default_center_lock_mode, default_cinematic_camera, default_clarity_profile,
    default_color_mode, default_contrast_profile, default_detail_profile,
    default_edge_accent_strength, default_false, default_fps_cap, default_freefly_speed,
    default_graphics_protocol, default_manual_cell_aspect, default_mode, default_model_lift,
    default_output_mode, default_perf_profile, default_reactive_gain, default_render_detail_mode,
    default_schema_version, default_stage_level, default_sync_hard_snap_ms, default_sync_kp,
    default_sync_policy, default_sync_speed_mode, default_texture_sampling, default_theme_style,
    default_true, default_wasd_mode, unix_now_secs,
};

pub(crate) const PRESET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub default_preset: Option<String>,
    pub last_used: Option<String>,
    #[serde(default)]
    pub presets: BTreeMap<String, WizardPreset>,
}

impl Default for PresetFile {
    fn default() -> Self {
        Self {
            schema_version: PRESET_SCHEMA_VERSION,
            default_preset: None,
            last_used: None,
            presets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardPreset {
    pub description: Option<String>,
    #[serde(default = "unix_now_secs")]
    pub created_at_unix: u64,
    #[serde(default = "unix_now_secs")]
    pub updated_at_unix: u64,
    #[serde(default)]
    pub assets: PresetAssets,
    #[serde(default)]
    pub render: PresetRender,
    #[serde(default)]
    pub visual: PresetVisual,
    #[serde(default)]
    pub sync: PresetSync,
    #[serde(default)]
    pub audio: PresetAudio,
    #[serde(default = "default_render_detail_mode")]
    pub render_detail_mode: String,
}

impl Default for WizardPreset {
    fn default() -> Self {
        let now = unix_now_secs();
        Self {
            description: None,
            created_at_unix: now,
            updated_at_unix: now,
            assets: PresetAssets::default(),
            render: PresetRender::default(),
            visual: PresetVisual::default(),
            sync: PresetSync::default(),
            audio: PresetAudio::default(),
            render_detail_mode: default_render_detail_mode(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetAssets {
    #[serde(default = "default_branch")]
    pub branch: String,
    pub model_name: Option<String>,
    pub motion_name: Option<String>,
    pub music_name: Option<String>,
    pub stage_name: Option<String>,
    pub camera_name: Option<String>,
}

impl Default for PresetAssets {
    fn default() -> Self {
        Self {
            branch: default_branch(),
            model_name: None,
            motion_name: None,
            music_name: None,
            stage_name: None,
            camera_name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetRender {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    #[serde(default = "default_graphics_protocol")]
    pub graphics_protocol: String,
    #[serde(default = "default_perf_profile")]
    pub perf_profile: String,
    #[serde(default = "default_detail_profile")]
    pub detail_profile: String,
    #[serde(default = "default_clarity_profile")]
    pub clarity_profile: String,
    #[serde(default = "default_ansi_quantization")]
    pub ansi_quantization: String,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_color_mode")]
    pub color_mode: String,
    #[serde(default = "default_braille_profile")]
    pub braille_profile: String,
    #[serde(default = "default_theme_style")]
    pub theme_style: String,
}

impl Default for PresetRender {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            output_mode: default_output_mode(),
            graphics_protocol: default_graphics_protocol(),
            perf_profile: default_perf_profile(),
            detail_profile: default_detail_profile(),
            clarity_profile: default_clarity_profile(),
            ansi_quantization: default_ansi_quantization(),
            backend: default_backend(),
            color_mode: default_color_mode(),
            braille_profile: default_braille_profile(),
            theme_style: default_theme_style(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetVisual {
    #[serde(default = "default_true")]
    pub center_lock: bool,
    #[serde(default = "default_center_lock_mode")]
    pub center_lock_mode: String,
    #[serde(default = "default_wasd_mode")]
    pub wasd_mode: String,
    #[serde(default = "default_freefly_speed")]
    pub freefly_speed: f32,
    #[serde(default = "default_camera_focus")]
    pub camera_focus: String,
    #[serde(default = "default_true")]
    pub material_color: bool,
    #[serde(default = "default_texture_sampling")]
    pub texture_sampling: String,
    #[serde(default = "default_model_lift")]
    pub model_lift: f32,
    #[serde(default = "default_edge_accent_strength")]
    pub edge_accent_strength: f32,
    #[serde(default = "default_braille_aspect_compensation")]
    pub braille_aspect_compensation: f32,
    #[serde(default = "default_stage_level")]
    pub stage_level: u8,
    #[serde(default = "default_true")]
    pub stage_reactive: bool,
    #[serde(default = "default_manual_cell_aspect")]
    pub manual_cell_aspect: f32,
    #[serde(default = "default_cell_aspect_mode")]
    pub cell_aspect_mode: String,
    #[serde(default = "default_cell_aspect_trim")]
    pub cell_aspect_trim: f32,
    #[serde(default = "default_contrast_profile")]
    pub contrast_profile: String,
    #[serde(default = "default_false")]
    pub font_preset_enabled: bool,
    #[serde(default = "default_camera_mode")]
    pub camera_mode: String,
    #[serde(default = "default_camera_align_preset")]
    pub camera_align_preset: String,
    #[serde(default = "default_camera_unit_scale")]
    pub camera_unit_scale: f32,
}

impl Default for PresetVisual {
    fn default() -> Self {
        Self {
            center_lock: default_true(),
            center_lock_mode: default_center_lock_mode(),
            wasd_mode: default_wasd_mode(),
            freefly_speed: default_freefly_speed(),
            camera_focus: default_camera_focus(),
            material_color: default_true(),
            texture_sampling: default_texture_sampling(),
            model_lift: default_model_lift(),
            edge_accent_strength: default_edge_accent_strength(),
            braille_aspect_compensation: default_braille_aspect_compensation(),
            stage_level: default_stage_level(),
            stage_reactive: default_true(),
            manual_cell_aspect: default_manual_cell_aspect(),
            cell_aspect_mode: default_cell_aspect_mode(),
            cell_aspect_trim: default_cell_aspect_trim(),
            contrast_profile: default_contrast_profile(),
            font_preset_enabled: default_false(),
            camera_mode: default_camera_mode(),
            camera_align_preset: default_camera_align_preset(),
            camera_unit_scale: default_camera_unit_scale(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSync {
    #[serde(default = "default_fps_cap")]
    pub fps_cap: u32,
    #[serde(default)]
    pub sync_offset_ms: i32,
    #[serde(default = "default_sync_speed_mode")]
    pub sync_speed_mode: String,
    #[serde(default = "default_sync_policy")]
    pub sync_policy: String,
    #[serde(default = "default_sync_hard_snap_ms")]
    pub sync_hard_snap_ms: u32,
    #[serde(default = "default_sync_kp")]
    pub sync_kp: f32,
}

impl Default for PresetSync {
    fn default() -> Self {
        Self {
            fps_cap: default_fps_cap(),
            sync_offset_ms: 0,
            sync_speed_mode: default_sync_speed_mode(),
            sync_policy: default_sync_policy(),
            sync_hard_snap_ms: default_sync_hard_snap_ms(),
            sync_kp: default_sync_kp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetAudio {
    #[serde(default = "default_audio_reactive")]
    pub audio_reactive: String,
    #[serde(default = "default_cinematic_camera")]
    pub cinematic_camera: String,
    #[serde(default = "default_reactive_gain")]
    pub reactive_gain: f32,
}

impl Default for PresetAudio {
    fn default() -> Self {
        Self {
            audio_reactive: default_audio_reactive(),
            cinematic_camera: default_cinematic_camera(),
            reactive_gain: default_reactive_gain(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePresetResult {
    Created,
    Overwritten,
    NameConflict,
}
