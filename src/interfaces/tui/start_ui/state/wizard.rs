use std::{collections::HashMap, path::PathBuf};

use crate::runtime::config::preset::{PresetStore, WizardPreset};

use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile,
    RenderBackend, RenderConfig, RenderMode, RenderOutputMode, StageQuality, SyncPolicy,
    SyncSpeedMode, TextureSamplingMode, ThemeStyle, resolve_cell_aspect,
};

use crate::interfaces::tui::helpers::{
    MIN_HEIGHT, MIN_WIDTH, START_FPS_OPTIONS, SYNC_OFFSET_LIMIT_MS, breakpoint_for,
    closest_u32_index, compute_duration_fit_factor, inspect_motion_duration,
};

use super::super::types::{
    ModelBranch, RenderDetailMode, StageChoice, StartSelection, StartWizardDefaults,
    StartWizardStep, UiBreakpoint,
};

#[cfg(feature = "gpu")]
use super::converters::gpu_available_once;
use super::entry::StartEntry;

#[derive(Debug, Clone)]
pub(crate) enum PresetPromptState {
    Inactive,
    EnterName { buffer: String },
    ConfirmOverwrite { name: String },
}

#[derive(Debug, Clone)]
pub(crate) struct StartWizardState {
    pub(crate) step: StartWizardStep,
    pub(crate) branch: ModelBranch,
    pub(crate) model_entries: Vec<StartEntry>,
    pub(crate) pmx_entries: Vec<StartEntry>,
    pub(crate) motion_entries: Vec<StartEntry>,
    pub(crate) music_entries: Vec<StartEntry>,
    pub(crate) stage_entries: Vec<StageChoice>,
    pub(crate) camera_entries: Vec<StartEntry>,
    pub(crate) model_index: usize,
    pub(crate) motion_index: usize,
    pub(crate) music_index: usize,
    pub(crate) stage_index: usize,
    pub(crate) camera_index: usize,
    pub(crate) mode: RenderMode,
    pub(crate) output_mode: RenderOutputMode,
    pub(crate) graphics_protocol: GraphicsProtocol,
    pub(crate) perf_profile: PerfProfile,
    pub(crate) detail_profile: DetailProfile,
    pub(crate) clarity_profile: ClarityProfile,
    pub(crate) ansi_quantization: AnsiQuantization,
    pub(crate) backend: RenderBackend,
    pub(crate) center_lock: bool,
    pub(crate) center_lock_mode: CenterLockMode,
    pub(crate) wasd_mode: CameraControlMode,
    pub(crate) freefly_speed: f32,
    pub(crate) camera_focus: CameraFocusMode,
    pub(crate) material_color: bool,
    pub(crate) texture_sampling: TextureSamplingMode,
    pub(crate) model_lift: f32,
    pub(crate) edge_accent_strength: f32,
    pub(crate) braille_aspect_compensation: f32,
    pub(crate) stage_level: u8,
    pub(crate) stage_reactive: bool,
    pub(crate) stage_quality: StageQuality,
    pub(crate) color_mode: ColorMode,
    pub(crate) braille_profile: BrailleProfile,
    pub(crate) theme_style: ThemeStyle,
    pub(crate) audio_reactive: AudioReactiveMode,
    pub(crate) cinematic_camera: CinematicCameraMode,
    pub(crate) reactive_gain: f32,
    pub(crate) fps_index: usize,
    pub(crate) manual_cell_aspect: f32,
    pub(crate) cell_aspect_mode: CellAspectMode,
    pub(crate) cell_aspect_trim: f32,
    pub(crate) contrast_profile: ContrastProfile,
    pub(crate) sync_offset_ms: i32,
    pub(crate) sync_speed_mode: SyncSpeedMode,
    pub(crate) sync_policy: SyncPolicy,
    pub(crate) sync_hard_snap_ms: u32,
    pub(crate) sync_kp: f32,
    pub(crate) font_preset_enabled: bool,
    pub(crate) camera_mode: CameraMode,
    pub(crate) camera_align_preset: CameraAlignPreset,
    pub(crate) camera_unit_scale: f32,
    pub(crate) camera_focus_index: usize,
    pub(crate) render_focus_index: usize,
    pub(crate) render_detail_mode: RenderDetailMode,
    pub(crate) preset_store: Option<PresetStore>,
    pub(crate) preset_names: Vec<String>,
    pub(crate) preset_index: usize,
    pub(crate) preset_default_name: Option<String>,
    pub(crate) preset_last_used_name: Option<String>,
    pub(crate) status_message: Option<String>,
    pub(crate) preset_prompt: PresetPromptState,
    pub(crate) pending_preset_save: Option<WizardPreset>,
    pub(crate) pending_preset_name: Option<String>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) detected_cell_aspect: Option<f32>,
    #[cfg(feature = "gpu")]
    pub(crate) gpu_available: bool,
    pub(crate) clip_duration_cache: HashMap<PathBuf, Option<f32>>,
    pub(crate) audio_duration_cache: HashMap<PathBuf, Option<f32>>,
}

impl StartWizardState {
    pub(crate) fn new(
        model_entries: Vec<StartEntry>,
        pmx_entries: Vec<StartEntry>,
        motion_entries: Vec<StartEntry>,
        music_entries: Vec<StartEntry>,
        stage_entries: Vec<StageChoice>,
        camera_entries: Vec<StartEntry>,
        preset_store: Option<PresetStore>,
        defaults: StartWizardDefaults,
        width: u16,
        height: u16,
    ) -> Self {
        let camera_index = defaults
            .camera_vmd_path
            .as_ref()
            .and_then(|selected| {
                camera_entries
                    .iter()
                    .position(|entry| entry.path == *selected)
                    .map(|idx| idx + 1)
            })
            .unwrap_or(0);
        Self {
            step: StartWizardStep::Branch,
            branch: ModelBranch::Glb,
            model_entries,
            pmx_entries,
            motion_entries,
            music_entries,
            stage_entries,
            camera_entries,
            model_index: 0,
            motion_index: 0,
            music_index: 0,
            stage_index: 0,
            camera_index,
            mode: defaults.mode,
            output_mode: defaults.output_mode,
            graphics_protocol: defaults.graphics_protocol,
            perf_profile: defaults.perf_profile,
            detail_profile: defaults.detail_profile,
            clarity_profile: defaults.clarity_profile,
            ansi_quantization: defaults.ansi_quantization,
            backend: defaults.backend,
            center_lock: defaults.center_lock,
            center_lock_mode: defaults.center_lock_mode,
            wasd_mode: defaults.wasd_mode,
            freefly_speed: defaults.freefly_speed.clamp(0.1, 8.0),
            camera_focus: defaults.camera_focus,
            material_color: defaults.material_color,
            texture_sampling: defaults.texture_sampling,
            model_lift: defaults.model_lift.clamp(0.02, 0.45),
            edge_accent_strength: defaults.edge_accent_strength.clamp(0.0, 1.5),
            braille_aspect_compensation: defaults.braille_aspect_compensation,
            stage_level: defaults.stage_level.min(4),
            stage_reactive: defaults.stage_reactive,
            stage_quality: defaults.stage_quality,
            color_mode: defaults.color_mode,
            braille_profile: defaults.braille_profile,
            theme_style: defaults.theme_style,
            audio_reactive: defaults.audio_reactive,
            cinematic_camera: defaults.cinematic_camera,
            reactive_gain: defaults.reactive_gain.clamp(0.0, 1.0),
            fps_index: closest_u32_index(defaults.fps_cap, &START_FPS_OPTIONS),
            manual_cell_aspect: defaults.cell_aspect,
            cell_aspect_mode: defaults.cell_aspect_mode,
            cell_aspect_trim: defaults.cell_aspect_trim.clamp(0.70, 1.30),
            contrast_profile: defaults.contrast_profile,
            sync_offset_ms: defaults
                .sync_offset_ms
                .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
            sync_speed_mode: defaults.sync_speed_mode,
            sync_policy: defaults.sync_policy,
            sync_hard_snap_ms: defaults.sync_hard_snap_ms.clamp(10, 2_000),
            sync_kp: defaults.sync_kp.clamp(0.01, 1.0),
            font_preset_enabled: defaults.font_preset_enabled,
            camera_mode: defaults.camera_mode,
            camera_align_preset: defaults.camera_align_preset,
            camera_unit_scale: defaults.camera_unit_scale.clamp(0.01, 2.0),
            camera_focus_index: 0,
            render_focus_index: 0,
            render_detail_mode: RenderDetailMode::Quick,
            preset_store,
            preset_names: Vec::new(),
            preset_index: 0,
            preset_default_name: None,
            preset_last_used_name: None,
            status_message: None,
            preset_prompt: PresetPromptState::Inactive,
            pending_preset_save: None,
            pending_preset_name: None,
            width,
            height,
            detected_cell_aspect: None,
            #[cfg(feature = "gpu")]
            gpu_available: gpu_available_once(),
            clip_duration_cache: HashMap::new(),
            audio_duration_cache: HashMap::new(),
        }
    }

    pub(crate) fn on_resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    pub(crate) fn initialize_presets(&mut self) {
        self.reload_preset_names();
        if !self.preset_names.is_empty() {
            self.apply_selected_preset_by_index();
        }
    }

    pub(crate) fn selection(&self) -> StartSelection {
        let active_model_path = self.selected_model_path().cloned().unwrap_or_default();
        let glb_path = active_model_path.clone();
        let pmx_path = if matches!(self.branch, ModelBranch::PmxVmd) {
            Some(active_model_path.clone())
        } else {
            None
        };
        let motion_vmd_path = self.selected_motion_path().cloned();
        let stage_choice = self.selected_stage_choice();
        let stage_transform = stage_choice
            .as_ref()
            .map(|choice| choice.transform)
            .unwrap_or_default();
        StartSelection {
            branch: self.branch,
            glb_path,
            pmx_path,
            motion_vmd_path,
            music_path: self.selected_music_path().cloned(),
            mode: self.mode,
            output_mode: self.output_mode,
            graphics_protocol: self.graphics_protocol,
            perf_profile: self.perf_profile,
            detail_profile: self.detail_profile,
            clarity_profile: self.clarity_profile,
            ansi_quantization: self.ansi_quantization,
            backend: self.backend,
            center_lock: self.center_lock,
            center_lock_mode: self.center_lock_mode,
            wasd_mode: self.wasd_mode,
            freefly_speed: self.freefly_speed,
            camera_focus: self.camera_focus,
            material_color: self.material_color,
            texture_sampling: self.texture_sampling,
            model_lift: self.model_lift,
            edge_accent_strength: self.edge_accent_strength,
            braille_aspect_compensation: self.braille_aspect_compensation,
            stage_level: self.stage_level,
            stage_reactive: self.stage_reactive,
            stage_quality: self.stage_quality,
            color_mode: if matches!(self.mode, RenderMode::Ascii) {
                ColorMode::Ansi
            } else {
                self.color_mode
            },
            braille_profile: self.braille_profile,
            theme_style: self.theme_style,
            audio_reactive: self.audio_reactive,
            cinematic_camera: self.cinematic_camera,
            reactive_gain: self.reactive_gain,
            fps_cap: START_FPS_OPTIONS[self.fps_index],
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
            sync_offset_ms: self.sync_offset_ms,
            sync_speed_mode: self.sync_speed_mode,
            sync_policy: self.sync_policy,
            sync_hard_snap_ms: self.sync_hard_snap_ms,
            sync_kp: self.sync_kp,
            stage_choice,
            stage_transform,
            apply_font_preset: self.font_preset_enabled,
            camera_vmd_path: self.selected_camera_path().cloned(),
            camera_mode: if self.camera_index == 0 {
                CameraMode::Off
            } else {
                self.camera_mode
            },
            camera_align_preset: self.camera_align_preset,
            camera_unit_scale: self.camera_unit_scale,
        }
    }

    pub(crate) fn selected_model_path(&self) -> Option<&PathBuf> {
        match self.branch {
            ModelBranch::Glb => self
                .model_entries
                .get(self.model_index)
                .map(|entry| &entry.path),
            ModelBranch::PmxVmd => self
                .pmx_entries
                .get(self.model_index)
                .map(|entry| &entry.path),
        }
    }

    pub(crate) fn selected_music_path(&self) -> Option<&PathBuf> {
        if self.music_index == 0 {
            None
        } else {
            self.music_entries
                .get(self.music_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(crate) fn selected_stage_choice(&self) -> Option<StageChoice> {
        if self.stage_index == 0 {
            None
        } else {
            self.stage_entries
                .get(self.stage_index.saturating_sub(1))
                .cloned()
        }
    }

    pub(crate) fn selected_camera_path(&self) -> Option<&PathBuf> {
        if self.camera_index == 0 {
            None
        } else {
            self.camera_entries
                .get(self.camera_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(crate) fn selected_motion_path(&self) -> Option<&PathBuf> {
        if !matches!(self.branch, ModelBranch::PmxVmd) || self.motion_index == 0 {
            None
        } else {
            self.motion_entries
                .get(self.motion_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(crate) fn selected_clip_duration_secs(&self) -> Option<f32> {
        match self.branch {
            ModelBranch::Glb => {
                let path = self.model_entries.get(self.model_index)?.path.clone();
                self.clip_duration_cache.get(&path).and_then(|value| *value)
            }
            ModelBranch::PmxVmd => self
                .selected_motion_path()
                .and_then(|path| inspect_motion_duration(path)),
        }
    }

    pub(crate) fn selected_audio_duration_secs(&self) -> Option<f32> {
        let path = self.selected_music_path()?.clone();
        self.audio_duration_cache
            .get(&path)
            .and_then(|value| *value)
    }

    pub(crate) fn expected_sync_speed(&self) -> f32 {
        compute_duration_fit_factor(
            self.selected_clip_duration_secs(),
            self.selected_audio_duration_secs(),
            self.sync_speed_mode,
        )
    }

    pub(crate) fn preview_render_config(&self) -> RenderConfig {
        RenderConfig {
            mode: self.mode,
            output_mode: self.output_mode,
            graphics_protocol: self.graphics_protocol,
            perf_profile: self.perf_profile,
            detail_profile: self.detail_profile,
            clarity_profile: self.clarity_profile,
            ansi_quantization: self.ansi_quantization,
            backend: self.backend,
            center_lock: self.center_lock,
            center_lock_mode: self.center_lock_mode,
            stage_level: self.stage_level,
            stage_reactive: self.stage_reactive,
            material_color: self.material_color,
            texture_sampling: self.texture_sampling,
            model_lift: self.model_lift,
            edge_accent_strength: self.edge_accent_strength,
            braille_aspect_compensation: self.braille_aspect_compensation,
            color_mode: if matches!(self.mode, RenderMode::Ascii) {
                ColorMode::Ansi
            } else {
                self.color_mode
            },
            ascii_force_color: true,
            braille_profile: self.braille_profile,
            theme_style: self.theme_style,
            audio_reactive: self.audio_reactive,
            cinematic_camera: self.cinematic_camera,
            camera_focus: self.camera_focus,
            reactive_gain: self.reactive_gain,
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
            sync_policy: self.sync_policy,
            sync_hard_snap_ms: self.sync_hard_snap_ms,
            sync_kp: self.sync_kp,
            ..RenderConfig::default()
        }
    }

    pub(crate) fn effective_cell_aspect(&self) -> f32 {
        resolve_cell_aspect(&self.preview_render_config(), self.detected_cell_aspect)
    }

    pub(crate) fn breakpoint(&self) -> UiBreakpoint {
        breakpoint_for(self.width, self.height)
    }

    pub(crate) fn is_too_small(&self) -> bool {
        self.width < MIN_WIDTH || self.height < MIN_HEIGHT
    }
}

#[derive(Debug, Clone)]
pub(crate) enum StartWizardAction {
    Continue,
    Cancel,
    Submit(Box<StartSelection>),
}
