use std::{collections::HashMap, fs, path::Path, path::PathBuf};

use glam::Vec3;

use crate::scene::{
    resolve_cell_aspect, AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset,
    CameraControlMode, CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode,
    CinematicCameraMode, ClarityProfile, ColorMode, ContrastProfile, DetailProfile,
    GraphicsProtocol, PerfProfile, RenderBackend, RenderConfig, RenderMode, RenderOutputMode,
    SyncPolicy, SyncSpeedMode, TextureSamplingMode, ThemeStyle,
};

use crate::runtime::start_ui_helpers::{
    breakpoint_for, closest_u32_index, compute_duration_fit_factor, detect_terminal_cell_aspect,
    format_mib, inspect_audio_duration, inspect_clip_duration, inspect_motion_duration, MIN_HEIGHT,
    MIN_WIDTH, START_FPS_OPTIONS, SYNC_OFFSET_LIMIT_MS,
};

use super::types::{
    ModelBranch, StageChoice, StartSelection, StartWizardDefaults, StartWizardEvent,
    StartWizardStep, UiBreakpoint,
};

#[derive(Debug, Clone)]
pub(super) struct StartEntry {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) bytes: u64,
}

impl StartEntry {
    pub(super) fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<invalid>")
            .to_owned();
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        Self {
            path: path.to_path_buf(),
            name,
            bytes,
        }
    }

    pub(super) fn label(&self) -> String {
        format!("{} ({})", self.name, format_mib(self.bytes))
    }
}

#[derive(Debug, Clone)]
pub(super) struct StartWizardState {
    pub(super) step: StartWizardStep,
    pub(super) branch: ModelBranch,
    pub(super) model_entries: Vec<StartEntry>,
    pub(super) pmx_entries: Vec<StartEntry>,
    pub(super) motion_entries: Vec<StartEntry>,
    pub(super) music_entries: Vec<StartEntry>,
    pub(super) stage_entries: Vec<StageChoice>,
    pub(super) camera_entries: Vec<StartEntry>,
    pub(super) model_index: usize,
    pub(super) motion_index: usize,
    pub(super) music_index: usize,
    pub(super) stage_index: usize,
    pub(super) camera_index: usize,
    pub(super) mode: RenderMode,
    pub(super) output_mode: RenderOutputMode,
    pub(super) graphics_protocol: GraphicsProtocol,
    pub(super) perf_profile: PerfProfile,
    pub(super) detail_profile: DetailProfile,
    pub(super) clarity_profile: ClarityProfile,
    pub(super) ansi_quantization: AnsiQuantization,
    pub(super) backend: RenderBackend,
    pub(super) center_lock: bool,
    pub(super) center_lock_mode: CenterLockMode,
    pub(super) wasd_mode: CameraControlMode,
    pub(super) freefly_speed: f32,
    pub(super) camera_focus: CameraFocusMode,
    pub(super) material_color: bool,
    pub(super) texture_sampling: TextureSamplingMode,
    pub(super) model_lift: f32,
    pub(super) edge_accent_strength: f32,
    pub(super) braille_aspect_compensation: f32,
    pub(super) stage_level: u8,
    pub(super) stage_reactive: bool,
    pub(super) color_mode: ColorMode,
    pub(super) braille_profile: BrailleProfile,
    pub(super) theme_style: ThemeStyle,
    pub(super) audio_reactive: AudioReactiveMode,
    pub(super) cinematic_camera: CinematicCameraMode,
    pub(super) reactive_gain: f32,
    pub(super) fps_index: usize,
    pub(super) manual_cell_aspect: f32,
    pub(super) cell_aspect_mode: CellAspectMode,
    pub(super) cell_aspect_trim: f32,
    pub(super) contrast_profile: ContrastProfile,
    pub(super) sync_offset_ms: i32,
    pub(super) sync_speed_mode: SyncSpeedMode,
    pub(super) sync_policy: SyncPolicy,
    pub(super) sync_hard_snap_ms: u32,
    pub(super) sync_kp: f32,
    pub(super) font_preset_enabled: bool,
    pub(super) camera_mode: CameraMode,
    pub(super) camera_align_preset: CameraAlignPreset,
    pub(super) camera_unit_scale: f32,
    pub(super) camera_focus_index: usize,
    pub(super) render_focus_index: usize,
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) detected_cell_aspect: Option<f32>,
    #[cfg(feature = "gpu")]
    pub(super) gpu_available: bool,
    pub(super) clip_duration_cache: HashMap<PathBuf, Option<f32>>,
    pub(super) audio_duration_cache: HashMap<PathBuf, Option<f32>>,
}

impl StartWizardState {
    pub(super) fn new(
        model_entries: Vec<StartEntry>,
        pmx_entries: Vec<StartEntry>,
        motion_entries: Vec<StartEntry>,
        music_entries: Vec<StartEntry>,
        stage_entries: Vec<StageChoice>,
        camera_entries: Vec<StartEntry>,
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
            width,
            height,
            detected_cell_aspect: None,
            #[cfg(feature = "gpu")]
            gpu_available: gpu_available_once(),
            clip_duration_cache: HashMap::new(),
            audio_duration_cache: HashMap::new(),
        }
    }

    pub(super) fn on_resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    pub(super) fn refresh_runtime_metrics(&mut self, anim_selector: Option<&str>) {
        self.detected_cell_aspect = detect_terminal_cell_aspect();

        let model_path = self
            .model_entries
            .get(self.model_index)
            .map(|entry| entry.path.clone());
        if let Some(path) = model_path {
            self.clip_duration_cache
                .entry(path.clone())
                .or_insert_with(|| inspect_clip_duration(&path, anim_selector));
        }

        let music_path = self.selected_music_path().cloned();
        if let Some(path) = music_path {
            self.audio_duration_cache
                .entry(path.clone())
                .or_insert_with(|| inspect_audio_duration(&path));
        }
    }

    pub(super) fn selection(&self) -> StartSelection {
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

    pub(super) fn selected_model_path(&self) -> Option<&PathBuf> {
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

    pub(super) fn selected_music_path(&self) -> Option<&PathBuf> {
        if self.music_index == 0 {
            None
        } else {
            self.music_entries
                .get(self.music_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(super) fn selected_stage_choice(&self) -> Option<StageChoice> {
        if self.stage_index == 0 {
            None
        } else {
            self.stage_entries
                .get(self.stage_index.saturating_sub(1))
                .cloned()
        }
    }

    pub(super) fn selected_camera_path(&self) -> Option<&PathBuf> {
        if self.camera_index == 0 {
            None
        } else {
            self.camera_entries
                .get(self.camera_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(super) fn selected_motion_path(&self) -> Option<&PathBuf> {
        if !matches!(self.branch, ModelBranch::PmxVmd) || self.motion_index == 0 {
            None
        } else {
            self.motion_entries
                .get(self.motion_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    pub(super) fn selected_clip_duration_secs(&self) -> Option<f32> {
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

    pub(super) fn selected_audio_duration_secs(&self) -> Option<f32> {
        let path = self.selected_music_path()?.clone();
        self.audio_duration_cache
            .get(&path)
            .and_then(|value| *value)
    }

    pub(super) fn expected_sync_speed(&self) -> f32 {
        compute_duration_fit_factor(
            self.selected_clip_duration_secs(),
            self.selected_audio_duration_secs(),
            self.sync_speed_mode,
        )
    }

    pub(super) fn preview_render_config(&self) -> RenderConfig {
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

    pub(super) fn effective_cell_aspect(&self) -> f32 {
        resolve_cell_aspect(&self.preview_render_config(), self.detected_cell_aspect)
    }

    pub(super) fn breakpoint(&self) -> UiBreakpoint {
        breakpoint_for(self.width, self.height)
    }

    pub(super) fn is_too_small(&self) -> bool {
        self.width < MIN_WIDTH || self.height < MIN_HEIGHT
    }
}

#[cfg(feature = "gpu")]
pub(super) fn gpu_available_once() -> bool {
    #[cfg(feature = "gpu")]
    {
        crate::render::gpu::GpuRenderer::is_available()
    }
}

#[derive(Debug, Clone)]
pub(super) enum StartWizardAction {
    Continue,
    Cancel,
    Submit(StartSelection),
}
