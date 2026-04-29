use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

const PRESET_SCHEMA_VERSION: u32 = 1;
const PRESET_NAME_MAX_LEN: usize = 48;

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

#[derive(Debug, Clone)]
pub struct PresetStore {
    path: PathBuf,
    file: PresetFile,
}

impl PresetStore {
    pub fn load_default() -> Result<Self> {
        let Some(path) = default_preset_store_path() else {
            bail!("failed to resolve user config directory")
        };
        Self::load(&path)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: path.to_path_buf(),
                    file: PresetFile::default(),
                });
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
        };

        let mut file: PresetFile =
            toml::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
        if file.schema_version == 0 {
            file.schema_version = PRESET_SCHEMA_VERSION;
        }

        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    pub fn list_names(&self) -> Vec<String> {
        self.file.presets.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<&WizardPreset> {
        self.file.presets.get(name)
    }

    pub fn default_preset(&self) -> Option<&str> {
        self.file.default_preset.as_deref()
    }

    pub fn last_used(&self) -> Option<&str> {
        self.file.last_used.as_deref()
    }

    pub fn has_preset(&self, name: &str) -> bool {
        self.file.presets.contains_key(name)
    }

    pub fn save_named(
        &mut self,
        name: &str,
        mut preset: WizardPreset,
        allow_overwrite: bool,
    ) -> Result<SavePresetResult> {
        let normalized = validate_preset_name(name)?;

        let result = if let Some(existing) = self.file.presets.get(&normalized) {
            if !allow_overwrite {
                return Ok(SavePresetResult::NameConflict);
            }
            preset.created_at_unix = existing.created_at_unix;
            SavePresetResult::Overwritten
        } else {
            SavePresetResult::Created
        };

        let now = unix_now_secs();
        if preset.created_at_unix == 0 {
            preset.created_at_unix = now;
        }
        preset.updated_at_unix = now;

        self.file.presets.insert(normalized.clone(), preset);
        self.file.last_used = Some(normalized.clone());
        if self
            .file
            .default_preset
            .as_deref()
            .is_some_and(|name| !self.file.presets.contains_key(name))
        {
            self.file.default_preset = None;
        }
        self.save_atomic()?;

        Ok(result)
    }

    pub fn set_last_used(&mut self, name: Option<String>) -> Result<()> {
        if let Some(ref key) = name && !self.file.presets.contains_key(key) {
            bail!("preset '{key}' not found")
        }
        self.file.last_used = name;
        self.save_atomic()
    }

    fn save_atomic(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let encoded = toml::to_string_pretty(&self.file).context("serialize preset file")?;
        let tmp = temp_path_for(&self.path);
        fs::write(&tmp, encoded).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }
}

pub fn validate_preset_name(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("preset name cannot be empty")
    }
    if trimmed.len() > PRESET_NAME_MAX_LEN {
        bail!("preset name must be <= {PRESET_NAME_MAX_LEN} characters")
    }
    if !trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.')
    }) {
        bail!("preset name may contain only letters, numbers, space, '-', '_' and '.'")
    }
    Ok(trimmed.to_owned())
}

pub fn default_preset_store_path() -> Option<PathBuf> {
    let base = BaseDirs::new()?;
    Some(base.config_dir().join("gascii").join("presets.toml"))
}

fn default_schema_version() -> u32 {
    PRESET_SCHEMA_VERSION
}

fn default_branch() -> String {
    "glb".to_owned()
}

fn default_mode() -> String {
    "braille".to_owned()
}

fn default_output_mode() -> String {
    "text".to_owned()
}

fn default_graphics_protocol() -> String {
    "auto".to_owned()
}

fn default_perf_profile() -> String {
    "balanced".to_owned()
}

fn default_detail_profile() -> String {
    "balanced".to_owned()
}

fn default_clarity_profile() -> String {
    "sharp".to_owned()
}

fn default_ansi_quantization() -> String {
    "q216".to_owned()
}

fn default_backend() -> String {
    "cpu".to_owned()
}

fn default_color_mode() -> String {
    "mono".to_owned()
}

fn default_braille_profile() -> String {
    "safe".to_owned()
}

fn default_theme_style() -> String {
    "theater".to_owned()
}

fn default_center_lock_mode() -> String {
    "root".to_owned()
}

fn default_wasd_mode() -> String {
    "freefly".to_owned()
}

fn default_freefly_speed() -> f32 {
    1.0
}

fn default_camera_focus() -> String {
    "auto".to_owned()
}

fn default_texture_sampling() -> String {
    "nearest".to_owned()
}

fn default_model_lift() -> f32 {
    0.12
}

fn default_edge_accent_strength() -> f32 {
    0.32
}

fn default_braille_aspect_compensation() -> f32 {
    1.0
}

fn default_stage_level() -> u8 {
    2
}

fn default_manual_cell_aspect() -> f32 {
    0.5
}

fn default_cell_aspect_mode() -> String {
    "auto".to_owned()
}

fn default_cell_aspect_trim() -> f32 {
    1.0
}

fn default_contrast_profile() -> String {
    "adaptive".to_owned()
}

fn default_camera_mode() -> String {
    "off".to_owned()
}

fn default_camera_align_preset() -> String {
    "std".to_owned()
}

fn default_camera_unit_scale() -> f32 {
    0.08
}

fn default_fps_cap() -> u32 {
    20
}

fn default_sync_speed_mode() -> String {
    "auto".to_owned()
}

fn default_sync_policy() -> String {
    "continuous".to_owned()
}

fn default_sync_hard_snap_ms() -> u32 {
    120
}

fn default_sync_kp() -> f32 {
    0.15
}

fn default_audio_reactive() -> String {
    "on".to_owned()
}

fn default_cinematic_camera() -> String {
    "on".to_owned()
}

fn default_reactive_gain() -> f32 {
    0.35
}

fn default_render_detail_mode() -> String {
    "quick".to_owned()
}

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn temp_path_for(path: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_name_validation_rejects_invalid_chars() {
        assert!(validate_preset_name("safe_name-01").is_ok());
        assert!(validate_preset_name("bad/name").is_err());
        assert!(validate_preset_name("    ").is_err());
    }

    #[test]
    fn preset_store_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("presets.toml");

        let mut store = PresetStore::load(&path).expect("load default");
        let mut preset = WizardPreset::default();
        preset.render.mode = "ascii".to_owned();
        let result = store
            .save_named("balanced", preset.clone(), false)
            .expect("save preset");
        assert_eq!(result, SavePresetResult::Created);

        let loaded = PresetStore::load(&path).expect("reload");
        assert_eq!(loaded.last_used(), Some("balanced"));
        assert_eq!(loaded.get("balanced").map(|p| p.render.mode.as_str()), Some("ascii"));
    }

    #[test]
    fn preset_store_requires_overwrite_confirmation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("presets.toml");

        let mut store = PresetStore::load(&path).expect("load default");
        let result = store
            .save_named("locked", WizardPreset::default(), false)
            .expect("save first");
        assert_eq!(result, SavePresetResult::Created);

        let conflict = store
            .save_named("locked", WizardPreset::default(), false)
            .expect("save second");
        assert_eq!(conflict, SavePresetResult::NameConflict);

        let overwrite = store
            .save_named("locked", WizardPreset::default(), true)
            .expect("overwrite");
        assert_eq!(overwrite, SavePresetResult::Overwritten);
    }
}
