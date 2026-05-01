use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use directories::BaseDirs;

const PRESET_NAME_MAX_LEN: usize = 48;
const PRESET_SCHEMA_VERSION: u32 = 1;

pub fn validate_preset_name(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("preset name cannot be empty")
    }
    if trimmed.len() > PRESET_NAME_MAX_LEN {
        bail!("preset name must be <= {PRESET_NAME_MAX_LEN} characters")
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
    {
        bail!("preset name may contain only letters, numbers, space, '-', '_' and '.'")
    }
    Ok(trimmed.to_owned())
}

pub fn default_preset_store_path() -> Option<PathBuf> {
    let base = BaseDirs::new()?;
    Some(base.config_dir().join("gascii").join("presets.toml"))
}

pub fn default_schema_version() -> u32 {
    PRESET_SCHEMA_VERSION
}

pub fn default_branch() -> String {
    "glb".to_owned()
}

pub fn default_mode() -> String {
    "braille".to_owned()
}

pub fn default_output_mode() -> String {
    "text".to_owned()
}

pub fn default_graphics_protocol() -> String {
    "auto".to_owned()
}

pub fn default_perf_profile() -> String {
    "balanced".to_owned()
}

pub fn default_detail_profile() -> String {
    "balanced".to_owned()
}

pub fn default_clarity_profile() -> String {
    "sharp".to_owned()
}

pub fn default_ansi_quantization() -> String {
    "q216".to_owned()
}

pub fn default_backend() -> String {
    "cpu".to_owned()
}

pub fn default_color_mode() -> String {
    "mono".to_owned()
}

pub fn default_braille_profile() -> String {
    "safe".to_owned()
}

pub fn default_theme_style() -> String {
    "theater".to_owned()
}

pub fn default_center_lock_mode() -> String {
    "root".to_owned()
}

pub fn default_wasd_mode() -> String {
    "freefly".to_owned()
}

pub fn default_freefly_speed() -> f32 {
    1.0
}

pub fn default_camera_focus() -> String {
    "auto".to_owned()
}

pub fn default_texture_sampling() -> String {
    "nearest".to_owned()
}

pub fn default_model_lift() -> f32 {
    0.12
}

pub fn default_edge_accent_strength() -> f32 {
    0.32
}

pub fn default_braille_aspect_compensation() -> f32 {
    1.0
}

pub fn default_stage_level() -> u8 {
    2
}

pub fn default_manual_cell_aspect() -> f32 {
    0.5
}

pub fn default_cell_aspect_mode() -> String {
    "auto".to_owned()
}

pub fn default_cell_aspect_trim() -> f32 {
    1.0
}

pub fn default_contrast_profile() -> String {
    "adaptive".to_owned()
}

pub fn default_camera_mode() -> String {
    "off".to_owned()
}

pub fn default_camera_align_preset() -> String {
    "std".to_owned()
}

pub fn default_camera_unit_scale() -> f32 {
    0.08
}

pub fn default_fps_cap() -> u32 {
    20
}

pub fn default_sync_speed_mode() -> String {
    "auto".to_owned()
}

pub fn default_sync_policy() -> String {
    "continuous".to_owned()
}

pub fn default_sync_hard_snap_ms() -> u32 {
    120
}

pub fn default_sync_kp() -> f32 {
    0.15
}

pub fn default_audio_reactive() -> String {
    "on".to_owned()
}

pub fn default_cinematic_camera() -> String {
    "on".to_owned()
}

pub fn default_reactive_gain() -> f32 {
    0.35
}

pub fn default_render_detail_mode() -> String {
    "quick".to_owned()
}

pub const fn default_true() -> bool {
    true
}

pub const fn default_false() -> bool {
    false
}

pub fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

pub fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("presets.toml");
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!("{file_name}.{pid}.{nonce}.tmp"))
}
