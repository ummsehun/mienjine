use crate::{
    runtime::state::{
        apply_adaptive_quality_tuning, apply_distant_subject_clarity_boost,
        apply_face_focus_detail_boost, apply_pmx_surface_guardrails, apply_runtime_contrast_preset,
    },
    scene::{AudioReactiveMode, CellAspectMode, RenderConfig},
};

use super::bootstrap::BootstrapState;

pub(super) fn build_frame_config(
    state: &mut BootstrapState,
    effective_aspect: f32,
) -> RenderConfig {
    let mut frame_config = state.config.clone();
    frame_config.cell_aspect_mode = CellAspectMode::Manual;
    frame_config.cell_aspect = effective_aspect;
    frame_config.center_lock = state.center_lock_enabled;
    frame_config.center_lock_mode = state.center_lock_mode;
    frame_config.stage_level = state.stage_level.min(4);
    frame_config.color_mode = crate::runtime::options::resolve_effective_color_mode(
        frame_config.mode,
        state.color_mode,
        state.ascii_force_color_active,
    );
    frame_config.ansi_quantization = state.ansi_quantization;
    frame_config.braille_profile = state.braille_profile;
    frame_config.cinematic_camera = state.cinematic_mode;
    frame_config.camera_focus = state.camera_focus_mode;
    frame_config.reactive_gain = state.reactive_gain;
    apply_runtime_contrast_preset(&mut frame_config, state.contrast_preset);

    let reactive_multiplier = match frame_config.audio_reactive {
        AudioReactiveMode::Off => 0.0,
        AudioReactiveMode::On => 1.0,
        AudioReactiveMode::High => 1.6,
    };
    let reactive_amount =
        (state.reactive_state.smoothed_energy * frame_config.reactive_gain * reactive_multiplier)
            .clamp(0.0, 1.0);
    frame_config.reactive_pulse = reactive_amount;
    if reactive_multiplier > 0.0 {
        let centered = state.reactive_state.smoothed_energy - 0.5;
        frame_config.contrast_floor = (frame_config.contrast_floor
            + centered * 0.04 * frame_config.reactive_gain)
            .clamp(0.04, 0.32);
        frame_config.fog_scale =
            (frame_config.fog_scale * (1.0 - reactive_amount * 0.18)).clamp(0.30, 1.5);
    }
    frame_config.exposure_bias =
        (state.exposure_bias + state.exposure_auto_boost.boost).clamp(-0.5, 0.8);

    apply_adaptive_quality_tuning(
        &mut frame_config,
        state.base_triangle_stride,
        state.base_min_triangle_area_px2,
        state.adaptive_quality.lod_level,
    );
    let prev_subject_height_ratio = if state.last_render_stats.subject_visible_height_ratio > 0.0 {
        state.last_render_stats.subject_visible_height_ratio
    } else {
        state.last_render_stats.visible_height_ratio
    };
    apply_distant_subject_clarity_boost(&mut frame_config, prev_subject_height_ratio);
    apply_face_focus_detail_boost(&mut frame_config, prev_subject_height_ratio);
    apply_pmx_surface_guardrails(
        &mut frame_config,
        state.is_pmx_scene,
        prev_subject_height_ratio,
    );

    frame_config
}
