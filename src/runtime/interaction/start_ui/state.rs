use std::{collections::HashMap, fs, path::Path, path::PathBuf};

use crate::runtime::config::{
    camera::{
        parse_camera_align_preset, parse_camera_focus, parse_camera_mode, parse_center_lock_mode,
        parse_texture_sampling, parse_wasd_mode,
    },
    general::{parse_cell_aspect_mode, parse_contrast_profile},
    preset::{PresetStore, SavePresetResult, WizardPreset},
    sync::{parse_sync_policy, parse_sync_speed_mode},
    visual::{
        parse_ansi_quantization, parse_audio_reactive, parse_backend, parse_braille_profile,
        parse_cinematic_camera, parse_clarity_profile, parse_color_mode, parse_detail_profile,
        parse_graphics_protocol, parse_output_mode, parse_perf_profile, parse_theme_style,
    },
};

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
    ModelBranch, RenderDetailMode, StageChoice, StartSelection, StartWizardDefaults,
    StartWizardStep, UiBreakpoint,
};

#[derive(Debug, Clone)]
pub(super) enum PresetPromptState {
    Inactive,
    EnterName { buffer: String },
    ConfirmOverwrite { name: String },
}

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
    pub(super) render_detail_mode: RenderDetailMode,
    pub(super) preset_store: Option<PresetStore>,
    pub(super) preset_names: Vec<String>,
    pub(super) preset_index: usize,
    pub(super) preset_default_name: Option<String>,
    pub(super) preset_last_used_name: Option<String>,
    pub(super) status_message: Option<String>,
    pub(super) preset_prompt: PresetPromptState,
    pub(super) pending_preset_save: Option<WizardPreset>,
    pub(super) pending_preset_name: Option<String>,
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

    pub(super) fn on_resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    pub(super) fn initialize_presets(&mut self) {
        self.reload_preset_names();
        if !self.preset_names.is_empty() {
            self.apply_selected_preset_by_index();
        }
    }

    pub(super) fn reload_preset_names(&mut self) {
        let Some(store) = self.preset_store.as_ref() else {
            self.preset_names.clear();
            self.preset_default_name = None;
            self.preset_last_used_name = None;
            self.preset_index = 0;
            return;
        };

        self.preset_names = store.list_names();
        self.preset_default_name = store.default_preset().map(str::to_owned);
        self.preset_last_used_name = store.last_used().map(str::to_owned);

        if let Some(last) = self.preset_last_used_name.as_ref()
            && let Some(index) = self.preset_names.iter().position(|name| name == last)
        {
            self.preset_index = index + 1;
            return;
        }
        self.preset_index = if self.preset_names.is_empty() { 0 } else { 1 };
    }

    pub(super) fn selected_preset_name(&self) -> Option<&str> {
        if self.preset_index == 0 {
            return None;
        }
        self.preset_names
            .get(self.preset_index.saturating_sub(1))
            .map(String::as_str)
    }

    pub(super) fn apply_selected_preset_by_index(&mut self) {
        let Some(name) = self.selected_preset_name().map(str::to_owned) else {
            return;
        };
        self.apply_preset_named(&name);
    }

    pub(super) fn apply_preset_named(&mut self, name: &str) {
        let Some(store) = self.preset_store.as_ref() else {
            self.status_message = Some("Preset store unavailable".to_owned());
            return;
        };
        let Some(preset) = store.get(name).cloned() else {
            self.status_message = Some(format!("Preset '{name}' not found"));
            return;
        };

        self.apply_wizard_preset(&preset);
        self.status_message = Some(format!("Preset loaded: {name}"));

        if let Some(store_mut) = self.preset_store.as_mut() {
            let _ = store_mut.set_last_used(Some(name.to_owned()));
        }
        self.preset_last_used_name = Some(name.to_owned());
    }

    pub(super) fn begin_preset_save_prompt(&mut self) {
        self.pending_preset_save = Some(self.build_wizard_preset_snapshot());
        self.pending_preset_name = None;
        self.preset_prompt = PresetPromptState::EnterName {
            buffer: String::new(),
        };
        self.status_message = Some("Preset name: type and press Enter".to_owned());
    }

    pub(super) fn save_pending_preset(&mut self, name: &str, allow_overwrite: bool) {
        let Some(preset) = self.pending_preset_save.clone() else {
            self.status_message = Some("No preset snapshot to save".to_owned());
            return;
        };
        let Some(store) = self.preset_store.as_mut() else {
            self.status_message = Some("Preset store unavailable".to_owned());
            return;
        };

        match store.save_named(name, preset, allow_overwrite) {
            Ok(SavePresetResult::Created) => {
                self.status_message = Some(format!("Preset saved: {name}"));
                self.preset_prompt = PresetPromptState::Inactive;
                self.pending_preset_name = None;
                self.pending_preset_save = None;
                self.reload_preset_names();
                if let Some(index) = self.preset_names.iter().position(|entry| entry == name) {
                    self.preset_index = index + 1;
                }
            }
            Ok(SavePresetResult::Overwritten) => {
                self.status_message = Some(format!("Preset overwritten: {name}"));
                self.preset_prompt = PresetPromptState::Inactive;
                self.pending_preset_name = None;
                self.pending_preset_save = None;
                self.reload_preset_names();
                if let Some(index) = self.preset_names.iter().position(|entry| entry == name) {
                    self.preset_index = index + 1;
                }
            }
            Ok(SavePresetResult::NameConflict) => {
                self.pending_preset_name = Some(name.to_owned());
                self.preset_prompt = PresetPromptState::ConfirmOverwrite {
                    name: name.to_owned(),
                };
                self.status_message = Some(format!(
                    "Preset '{name}' exists. Enter=overwrite, Esc=cancel"
                ));
            }
            Err(error) => {
                self.status_message = Some(format!("Preset save failed: {error}"));
            }
        }
    }

    pub(super) fn cancel_preset_prompt(&mut self) {
        self.preset_prompt = PresetPromptState::Inactive;
        self.pending_preset_name = None;
        self.pending_preset_save = None;
        self.status_message = Some("Preset save canceled".to_owned());
    }

    pub(super) fn build_wizard_preset_snapshot(&self) -> WizardPreset {
        let selection = self.selection();
        let mut preset = WizardPreset::default();
        preset.assets.branch = match selection.branch {
            ModelBranch::Glb => "glb",
            ModelBranch::PmxVmd => "pmx-vmd",
        }
        .to_owned();
        preset.assets.model_name = selection
            .glb_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        preset.assets.motion_name = selection
            .motion_vmd_path
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        preset.assets.music_name = selection
            .music_path
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        preset.assets.stage_name = selection.stage_choice.as_ref().map(|stage| stage.name.clone());
        preset.assets.camera_name = selection
            .camera_vmd_path
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned);

        preset.render.mode = mode_to_text(selection.mode);
        preset.render.output_mode = output_mode_to_text(selection.output_mode);
        preset.render.graphics_protocol = graphics_protocol_to_text(selection.graphics_protocol);
        preset.render.perf_profile = perf_profile_to_text(selection.perf_profile);
        preset.render.detail_profile = detail_profile_to_text(selection.detail_profile);
        preset.render.clarity_profile = clarity_profile_to_text(selection.clarity_profile);
        preset.render.ansi_quantization = ansi_quantization_to_text(selection.ansi_quantization);
        preset.render.backend = backend_to_text(selection.backend);
        preset.render.color_mode = color_mode_to_text(selection.color_mode);
        preset.render.braille_profile = braille_profile_to_text(selection.braille_profile);
        preset.render.theme_style = theme_style_to_text(selection.theme_style);

        preset.visual.center_lock = selection.center_lock;
        preset.visual.center_lock_mode = center_lock_mode_to_text(selection.center_lock_mode);
        preset.visual.wasd_mode = wasd_mode_to_text(selection.wasd_mode);
        preset.visual.freefly_speed = selection.freefly_speed;
        preset.visual.camera_focus = camera_focus_to_text(selection.camera_focus);
        preset.visual.material_color = selection.material_color;
        preset.visual.texture_sampling = texture_sampling_to_text(selection.texture_sampling);
        preset.visual.model_lift = selection.model_lift;
        preset.visual.edge_accent_strength = selection.edge_accent_strength;
        preset.visual.braille_aspect_compensation = selection.braille_aspect_compensation;
        preset.visual.stage_level = selection.stage_level;
        preset.visual.stage_reactive = selection.stage_reactive;
        preset.visual.manual_cell_aspect = selection.cell_aspect;
        preset.visual.cell_aspect_mode = cell_aspect_mode_to_text(selection.cell_aspect_mode);
        preset.visual.cell_aspect_trim = selection.cell_aspect_trim;
        preset.visual.contrast_profile = contrast_profile_to_text(selection.contrast_profile);
        preset.visual.font_preset_enabled = selection.apply_font_preset;
        preset.visual.camera_mode = camera_mode_to_text(selection.camera_mode);
        preset.visual.camera_align_preset = camera_align_preset_to_text(selection.camera_align_preset);
        preset.visual.camera_unit_scale = selection.camera_unit_scale;

        preset.sync.fps_cap = selection.fps_cap;
        preset.sync.sync_offset_ms = selection.sync_offset_ms;
        preset.sync.sync_speed_mode = sync_speed_mode_to_text(selection.sync_speed_mode);
        preset.sync.sync_policy = sync_policy_to_text(selection.sync_policy);
        preset.sync.sync_hard_snap_ms = selection.sync_hard_snap_ms;
        preset.sync.sync_kp = selection.sync_kp;

        preset.audio.audio_reactive = audio_reactive_to_text(selection.audio_reactive);
        preset.audio.cinematic_camera = cinematic_camera_to_text(selection.cinematic_camera);
        preset.audio.reactive_gain = selection.reactive_gain;

        preset.render_detail_mode = render_detail_mode_to_text(self.render_detail_mode);

        preset
    }

    pub(super) fn apply_wizard_preset(&mut self, preset: &WizardPreset) {
        self.mode = parse_mode_text(&preset.render.mode);
        self.output_mode = parse_output_mode(&preset.render.output_mode);
        self.graphics_protocol = parse_graphics_protocol(&preset.render.graphics_protocol);
        self.perf_profile = parse_perf_profile(&preset.render.perf_profile);
        self.detail_profile = parse_detail_profile(&preset.render.detail_profile);
        self.clarity_profile = parse_clarity_profile(&preset.render.clarity_profile);
        self.ansi_quantization = parse_ansi_quantization(&preset.render.ansi_quantization);
        self.backend = parse_backend(&preset.render.backend);
        self.color_mode = parse_color_mode(&preset.render.color_mode).unwrap_or(ColorMode::Mono);
        self.braille_profile = parse_braille_profile(&preset.render.braille_profile);
        self.theme_style = parse_theme_style(&preset.render.theme_style);

        self.center_lock = preset.visual.center_lock;
        self.center_lock_mode = parse_center_lock_mode(&preset.visual.center_lock_mode);
        self.wasd_mode = parse_wasd_mode(&preset.visual.wasd_mode);
        self.freefly_speed = preset.visual.freefly_speed.clamp(0.1, 8.0);
        self.camera_focus = parse_camera_focus(&preset.visual.camera_focus);
        self.material_color = preset.visual.material_color;
        self.texture_sampling = parse_texture_sampling(&preset.visual.texture_sampling);
        self.model_lift = preset.visual.model_lift.clamp(0.02, 0.45);
        self.edge_accent_strength = preset.visual.edge_accent_strength.clamp(0.0, 1.5);
        self.braille_aspect_compensation = preset.visual.braille_aspect_compensation.clamp(0.70, 1.30);
        self.stage_level = preset.visual.stage_level.min(4);
        self.stage_reactive = preset.visual.stage_reactive;
        self.manual_cell_aspect = preset.visual.manual_cell_aspect;
        self.cell_aspect_mode = parse_cell_aspect_mode(&preset.visual.cell_aspect_mode);
        self.cell_aspect_trim = preset.visual.cell_aspect_trim.clamp(0.70, 1.30);
        self.contrast_profile = parse_contrast_profile(&preset.visual.contrast_profile);
        self.font_preset_enabled = preset.visual.font_preset_enabled;
        self.camera_mode = parse_camera_mode(&preset.visual.camera_mode);
        self.camera_align_preset = parse_camera_align_preset(&preset.visual.camera_align_preset);
        self.camera_unit_scale = preset.visual.camera_unit_scale.clamp(0.01, 2.0);

        self.fps_index = closest_u32_index(preset.sync.fps_cap, &START_FPS_OPTIONS);
        self.sync_offset_ms = preset
            .sync
            .sync_offset_ms
            .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
        self.sync_speed_mode = parse_sync_speed_mode(&preset.sync.sync_speed_mode);
        self.sync_policy = parse_sync_policy(&preset.sync.sync_policy);
        self.sync_hard_snap_ms = preset.sync.sync_hard_snap_ms.clamp(10, 2_000);
        self.sync_kp = preset.sync.sync_kp.clamp(0.01, 1.0);

        self.audio_reactive = parse_audio_reactive(&preset.audio.audio_reactive);
        self.cinematic_camera = parse_cinematic_camera(&preset.audio.cinematic_camera);
        self.reactive_gain = preset.audio.reactive_gain.clamp(0.0, 1.0);

        self.render_detail_mode = parse_render_detail_mode_text(&preset.render_detail_mode);
        let count = self.current_render_field_count();
        self.render_focus_index = self.render_focus_index.min(count.saturating_sub(1));

        self.apply_preset_assets(preset);
    }

    fn apply_preset_assets(&mut self, preset: &WizardPreset) {
        self.branch = if preset.assets.branch.to_ascii_lowercase().starts_with("pmx") {
            ModelBranch::PmxVmd
        } else {
            ModelBranch::Glb
        };

        if let Some(ref name) = preset.assets.model_name {
            let entries = if matches!(self.branch, ModelBranch::PmxVmd) {
                &self.pmx_entries
            } else {
                &self.model_entries
            };
            if let Some(index) = entries.iter().position(|entry| entry.name == *name) {
                self.model_index = index;
            }
        }
        if let Some(ref name) = preset.assets.motion_name {
            if let Some(index) = self.motion_entries.iter().position(|entry| entry.name == *name) {
                self.motion_index = index + 1;
            }
        }
        if let Some(ref name) = preset.assets.music_name {
            if let Some(index) = self.music_entries.iter().position(|entry| entry.name == *name) {
                self.music_index = index + 1;
            }
        }
        if let Some(ref name) = preset.assets.stage_name {
            if let Some(index) = self.stage_entries.iter().position(|entry| entry.name == *name) {
                self.stage_index = index + 1;
            }
        }
        if let Some(ref name) = preset.assets.camera_name {
            if let Some(index) = self.camera_entries.iter().position(|entry| entry.name == *name) {
                self.camera_index = index + 1;
            }
        }
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

fn mode_to_text(value: RenderMode) -> String {
    match value {
        RenderMode::Ascii => "ascii",
        RenderMode::Braille => "braille",
    }
    .to_owned()
}

fn parse_mode_text(value: &str) -> RenderMode {
    if value.to_ascii_lowercase().starts_with("asc") {
        RenderMode::Ascii
    } else {
        RenderMode::Braille
    }
}

fn output_mode_to_text(value: RenderOutputMode) -> String {
    match value {
        RenderOutputMode::Text => "text",
        RenderOutputMode::Hybrid => "hybrid",
        RenderOutputMode::KittyHq => "kitty-hq",
    }
    .to_owned()
}

fn graphics_protocol_to_text(value: GraphicsProtocol) -> String {
    match value {
        GraphicsProtocol::Auto => "auto",
        GraphicsProtocol::Kitty => "kitty",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    }
    .to_owned()
}

fn perf_profile_to_text(value: PerfProfile) -> String {
    match value {
        PerfProfile::Balanced => "balanced",
        PerfProfile::Cinematic => "cinematic",
        PerfProfile::Smooth => "smooth",
    }
    .to_owned()
}

fn detail_profile_to_text(value: DetailProfile) -> String {
    match value {
        DetailProfile::Perf => "perf",
        DetailProfile::Balanced => "balanced",
        DetailProfile::Ultra => "ultra",
    }
    .to_owned()
}

fn clarity_profile_to_text(value: ClarityProfile) -> String {
    match value {
        ClarityProfile::Balanced => "balanced",
        ClarityProfile::Sharp => "sharp",
        ClarityProfile::Extreme => "extreme",
    }
    .to_owned()
}

fn ansi_quantization_to_text(value: AnsiQuantization) -> String {
    match value {
        AnsiQuantization::Q216 => "q216",
        AnsiQuantization::Off => "off",
    }
    .to_owned()
}

fn backend_to_text(value: RenderBackend) -> String {
    match value {
        RenderBackend::Cpu => "cpu",
        RenderBackend::Gpu => "gpu",
    }
    .to_owned()
}

fn color_mode_to_text(value: ColorMode) -> String {
    match value {
        ColorMode::Mono => "mono",
        ColorMode::Ansi => "ansi",
    }
    .to_owned()
}

fn braille_profile_to_text(value: BrailleProfile) -> String {
    match value {
        BrailleProfile::Safe => "safe",
        BrailleProfile::Normal => "normal",
        BrailleProfile::Dense => "dense",
    }
    .to_owned()
}

fn theme_style_to_text(value: ThemeStyle) -> String {
    match value {
        ThemeStyle::Theater => "theater",
        ThemeStyle::Neon => "neon",
        ThemeStyle::Holo => "holo",
    }
    .to_owned()
}

fn center_lock_mode_to_text(value: CenterLockMode) -> String {
    match value {
        CenterLockMode::Root => "root",
        CenterLockMode::Mixed => "mixed",
    }
    .to_owned()
}

fn wasd_mode_to_text(value: CameraControlMode) -> String {
    match value {
        CameraControlMode::Orbit => "orbit",
        CameraControlMode::FreeFly => "freefly",
    }
    .to_owned()
}

fn camera_focus_to_text(value: CameraFocusMode) -> String {
    match value {
        CameraFocusMode::Auto => "auto",
        CameraFocusMode::Full => "full",
        CameraFocusMode::Upper => "upper",
        CameraFocusMode::Face => "face",
        CameraFocusMode::Hands => "hands",
    }
    .to_owned()
}

fn texture_sampling_to_text(value: TextureSamplingMode) -> String {
    match value {
        TextureSamplingMode::Nearest => "nearest",
        TextureSamplingMode::Bilinear => "bilinear",
    }
    .to_owned()
}

fn cell_aspect_mode_to_text(value: CellAspectMode) -> String {
    match value {
        CellAspectMode::Auto => "auto",
        CellAspectMode::Manual => "manual",
    }
    .to_owned()
}

fn contrast_profile_to_text(value: ContrastProfile) -> String {
    match value {
        ContrastProfile::Adaptive => "adaptive",
        ContrastProfile::Fixed => "fixed",
    }
    .to_owned()
}

fn camera_mode_to_text(value: CameraMode) -> String {
    match value {
        CameraMode::Off => "off",
        CameraMode::Vmd => "vmd",
        CameraMode::Blend => "blend",
    }
    .to_owned()
}

fn camera_align_preset_to_text(value: CameraAlignPreset) -> String {
    match value {
        CameraAlignPreset::Std => "std",
        CameraAlignPreset::AltA => "alt-a",
        CameraAlignPreset::AltB => "alt-b",
    }
    .to_owned()
}

fn sync_speed_mode_to_text(value: SyncSpeedMode) -> String {
    match value {
        SyncSpeedMode::AutoDurationFit => "auto",
        SyncSpeedMode::Realtime1x => "realtime",
    }
    .to_owned()
}

fn sync_policy_to_text(value: SyncPolicy) -> String {
    match value {
        SyncPolicy::Continuous => "continuous",
        SyncPolicy::Fixed => "fixed",
        SyncPolicy::Manual => "manual",
    }
    .to_owned()
}

fn audio_reactive_to_text(value: AudioReactiveMode) -> String {
    match value {
        AudioReactiveMode::Off => "off",
        AudioReactiveMode::On => "on",
        AudioReactiveMode::High => "high",
    }
    .to_owned()
}

fn cinematic_camera_to_text(value: CinematicCameraMode) -> String {
    match value {
        CinematicCameraMode::Off => "off",
        CinematicCameraMode::On => "on",
        CinematicCameraMode::Aggressive => "aggressive",
    }
    .to_owned()
}

fn render_detail_mode_to_text(value: RenderDetailMode) -> String {
    match value {
        RenderDetailMode::Quick => "quick",
        RenderDetailMode::Advanced => "advanced",
    }
    .to_owned()
}

fn parse_render_detail_mode_text(value: &str) -> RenderDetailMode {
    if value.to_ascii_lowercase().starts_with("adv") {
        RenderDetailMode::Advanced
    } else {
        RenderDetailMode::Quick
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
