use crate::runtime::config::{
    camera::{
        parse_camera_align_preset, parse_camera_focus, parse_camera_mode, parse_center_lock_mode,
        parse_texture_sampling, parse_wasd_mode,
    },
    general::{parse_cell_aspect_mode, parse_contrast_profile},
    preset::{SavePresetResult, WizardPreset},
    sync::{parse_sync_policy, parse_sync_speed_mode},
    visual::{
        parse_ansi_quantization, parse_audio_reactive, parse_backend, parse_braille_profile,
        parse_cinematic_camera, parse_clarity_profile, parse_color_mode, parse_detail_profile,
        parse_graphics_protocol, parse_output_mode, parse_perf_profile, parse_theme_style,
    },
};

use crate::scene::ColorMode;

use super::super::types::ModelBranch;
use super::converters::{
    ansi_quantization_to_text, audio_reactive_to_text, backend_to_text, braille_profile_to_text,
    camera_align_preset_to_text, camera_focus_to_text, camera_mode_to_text,
    cell_aspect_mode_to_text, center_lock_mode_to_text, cinematic_camera_to_text,
    clarity_profile_to_text, color_mode_to_text, contrast_profile_to_text, detail_profile_to_text,
    graphics_protocol_to_text, mode_to_text, output_mode_to_text, parse_mode_text,
    parse_render_detail_mode_text, perf_profile_to_text, render_detail_mode_to_text,
    sync_policy_to_text, sync_speed_mode_to_text, texture_sampling_to_text, theme_style_to_text,
    wasd_mode_to_text,
};
use super::wizard::{PresetPromptState, StartWizardState};
use crate::interfaces::tui::helpers::{START_FPS_OPTIONS, SYNC_OFFSET_LIMIT_MS, closest_u32_index};

impl StartWizardState {
    pub(crate) fn reload_preset_names(&mut self) {
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

    pub(crate) fn selected_preset_name(&self) -> Option<&str> {
        if self.preset_index == 0 {
            return None;
        }
        self.preset_names
            .get(self.preset_index.saturating_sub(1))
            .map(String::as_str)
    }

    pub(crate) fn apply_selected_preset_by_index(&mut self) {
        let Some(name) = self.selected_preset_name().map(str::to_owned) else {
            return;
        };
        self.apply_preset_named(&name);
    }

    pub(crate) fn apply_preset_named(&mut self, name: &str) {
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

    pub(crate) fn begin_preset_save_prompt(&mut self) {
        self.pending_preset_save = Some(self.build_wizard_preset_snapshot());
        self.pending_preset_name = None;
        self.preset_prompt = PresetPromptState::EnterName {
            buffer: String::new(),
        };
        self.status_message = Some("Preset name: type and press Enter".to_owned());
    }

    pub(crate) fn save_pending_preset(&mut self, name: &str, allow_overwrite: bool) {
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

    pub(crate) fn cancel_preset_prompt(&mut self) {
        self.preset_prompt = PresetPromptState::Inactive;
        self.pending_preset_name = None;
        self.pending_preset_save = None;
        self.status_message = Some("Preset save canceled".to_owned());
    }

    pub(crate) fn build_wizard_preset_snapshot(&self) -> WizardPreset {
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
        preset.assets.stage_name = selection
            .stage_choice
            .as_ref()
            .map(|stage| stage.name.clone());
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
        preset.visual.camera_align_preset =
            camera_align_preset_to_text(selection.camera_align_preset);
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

    pub(crate) fn apply_wizard_preset(&mut self, preset: &WizardPreset) {
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
        self.braille_aspect_compensation =
            preset.visual.braille_aspect_compensation.clamp(0.70, 1.30);
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
        if let Some(ref name) = preset.assets.motion_name
            && let Some(index) = self
                .motion_entries
                .iter()
                .position(|entry| entry.name == *name)
        {
            self.motion_index = index + 1;
        }
        if let Some(ref name) = preset.assets.music_name
            && let Some(index) = self
                .music_entries
                .iter()
                .position(|entry| entry.name == *name)
        {
            self.music_index = index + 1;
        }
        if let Some(ref name) = preset.assets.stage_name
            && let Some(index) = self
                .stage_entries
                .iter()
                .position(|entry| entry.name == *name)
        {
            self.stage_index = index + 1;
        }
        if let Some(ref name) = preset.assets.camera_name
            && let Some(index) = self
                .camera_entries
                .iter()
                .position(|entry| entry.name == *name)
        {
            self.camera_index = index + 1;
        }
    }
}
