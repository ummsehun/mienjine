use crate::{
    cli::StartArgs,
    runtime::{
        app_render_config::render_config_from_start,
        config::GasciiConfig,
        options::{
            ResolvedSyncOptions, ResolvedVisualOptions, resolve_effective_camera_mode,
            resolve_effective_color_mode,
        },
        state::RuntimeCameraSettings,
    },
    scene::{RenderConfig, RenderMode},
};

/// Built render configuration and camera settings for the render loop.
pub(super) struct RenderBuild {
    pub config: RenderConfig,
    pub camera_settings: RuntimeCameraSettings,
    pub wasd_mode: crate::scene::CameraControlMode,
    pub freefly_speed: f32,
}

/// Build the full render configuration from the wizard selection.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_render_config(
    selection: &crate::interfaces::tui::start_ui::StartSelection,
    args: &StartArgs,
    visual: &ResolvedVisualOptions,
    effective_sync: &ResolvedSyncOptions,
    runtime_cfg: &GasciiConfig,
    _start_mode: RenderMode,
) -> RenderBuild {
    let mut config = render_config_from_start(
        args,
        &ResolvedVisualOptions {
            output_mode: selection.output_mode,
            recover_color_auto: visual.recover_color_auto,
            graphics_protocol: selection.graphics_protocol,
            kitty_transport: visual.kitty_transport,
            kitty_compression: visual.kitty_compression,
            kitty_internal_res: visual.kitty_internal_res,
            kitty_pipeline_mode: visual.kitty_pipeline_mode,
            recover_strategy: visual.recover_strategy,
            kitty_scale: visual.kitty_scale,
            hq_target_fps: visual.hq_target_fps,
            subject_exposure_only: visual.subject_exposure_only,
            subject_target_height_ratio: visual.subject_target_height_ratio,
            subject_target_width_ratio: visual.subject_target_width_ratio,
            quality_auto_distance: visual.quality_auto_distance,
            texture_mip_bias: visual.texture_mip_bias,
            stage_as_sub_only: visual.stage_as_sub_only,
            stage_role: visual.stage_role,
            stage_quality: selection.stage_quality,
            stage_luma_cap: visual.stage_luma_cap,
            cell_aspect_mode: selection.cell_aspect_mode,
            cell_aspect_trim: selection.cell_aspect_trim,
            contrast_profile: selection.contrast_profile,
            perf_profile: selection.perf_profile,
            detail_profile: selection.detail_profile,
            backend: selection.backend,
            exposure_bias: visual.exposure_bias,
            center_lock: selection.center_lock,
            center_lock_mode: selection.center_lock_mode,
            wasd_mode: selection.wasd_mode,
            freefly_speed: selection.freefly_speed,
            camera_look_speed: visual.camera_look_speed,
            camera_mode: selection.camera_mode,
            camera_align_preset: selection.camera_align_preset,
            camera_unit_scale: selection.camera_unit_scale,
            camera_vmd_fps: visual.camera_vmd_fps,
            camera_vmd_path: selection.camera_vmd_path.clone(),
            camera_focus: selection.camera_focus,
            material_color: selection.material_color,
            texture_sampling: selection.texture_sampling,
            texture_v_origin: visual.texture_v_origin,
            texture_sampler: visual.texture_sampler,
            clarity_profile: selection.clarity_profile,
            ansi_quantization: selection.ansi_quantization,
            model_lift: selection.model_lift,
            edge_accent_strength: selection.edge_accent_strength,
            bg_suppression: visual.bg_suppression,
            braille_aspect_compensation: selection.braille_aspect_compensation,
            stage_level: selection.stage_level,
            stage_reactive: selection.stage_reactive,
            color_mode: Some(selection.color_mode),
            ascii_force_color: visual.ascii_force_color,
            braille_profile: selection.braille_profile,
            theme_style: selection.theme_style,
            audio_reactive: selection.audio_reactive,
            cinematic_camera: selection.cinematic_camera,
            reactive_gain: selection.reactive_gain,
        },
    );

    config.mode = selection.mode;
    config.output_mode = selection.output_mode;
    config.graphics_protocol = selection.graphics_protocol;
    config.perf_profile = selection.perf_profile;
    config.detail_profile = selection.detail_profile;
    config.backend = selection.backend;
    config.color_mode =
        resolve_effective_color_mode(config.mode, selection.color_mode, config.ascii_force_color);
    config.braille_profile = selection.braille_profile;
    config.theme_style = selection.theme_style;
    config.audio_reactive = selection.audio_reactive;
    config.cinematic_camera = selection.cinematic_camera;
    config.camera_focus = selection.camera_focus;
    config.reactive_gain = selection.reactive_gain;
    config.fps_cap = selection.fps_cap;
    config.cell_aspect = selection.cell_aspect;
    config.center_lock = selection.center_lock;
    config.center_lock_mode = selection.center_lock_mode;

    let wasd_mode = selection.wasd_mode;
    let freefly_speed = selection.freefly_speed;

    let effective_camera_mode =
        resolve_effective_camera_mode(selection.camera_mode, selection.camera_vmd_path.is_some());

    let camera_settings = RuntimeCameraSettings {
        mode: effective_camera_mode,
        align_preset: selection.camera_align_preset,
        unit_scale: selection.camera_unit_scale,
        vmd_fps: visual.camera_vmd_fps,
        vmd_path: selection.camera_vmd_path.clone(),
        look_speed: visual.camera_look_speed,
    };

    config.stage_level = selection.stage_level;
    config.stage_reactive = selection.stage_reactive;
    config.material_color = selection.material_color;
    config.texture_sampling = selection.texture_sampling;
    config.clarity_profile = selection.clarity_profile;
    config.ansi_quantization = selection.ansi_quantization;
    config.model_lift = selection.model_lift;
    config.edge_accent_strength = selection.edge_accent_strength;
    config.braille_aspect_compensation = selection.braille_aspect_compensation;
    config.sync_policy = effective_sync.sync_policy;
    config.sync_hard_snap_ms = effective_sync.sync_hard_snap_ms;
    config.sync_kp = effective_sync.sync_kp;

    crate::runtime::app::apply_runtime_render_tuning(&mut config, runtime_cfg);

    RenderBuild {
        config,
        camera_settings,
        wasd_mode,
        freefly_speed,
    }
}
