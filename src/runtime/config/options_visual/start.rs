use crate::{
    cli::StartArgs,
    runtime::{config::GasciiConfig, options::ResolvedVisualOptions},
};

pub(crate) fn resolve_visual_options_for_start(
    args: &StartArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedVisualOptions {
    ResolvedVisualOptions {
        output_mode: args
            .output_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.output_mode),
        recover_color_auto: args
            .recover_color
            .map(Into::into)
            .unwrap_or(runtime_cfg.recover_color_auto),
        graphics_protocol: args
            .graphics_protocol
            .map(Into::into)
            .unwrap_or(runtime_cfg.graphics_protocol),
        kitty_transport: args
            .kitty_transport
            .map(Into::into)
            .unwrap_or(runtime_cfg.kitty_transport),
        kitty_compression: args
            .kitty_compression
            .map(Into::into)
            .unwrap_or(runtime_cfg.kitty_compression),
        kitty_internal_res: args
            .kitty_internal_res
            .map(Into::into)
            .unwrap_or(runtime_cfg.kitty_internal_res),
        kitty_pipeline_mode: args
            .kitty_pipeline
            .map(Into::into)
            .unwrap_or(runtime_cfg.kitty_pipeline_mode),
        recover_strategy: args
            .recover_strategy
            .map(Into::into)
            .unwrap_or(runtime_cfg.recover_strategy),
        kitty_scale: args
            .kitty_scale
            .unwrap_or(runtime_cfg.kitty_scale)
            .clamp(0.5, 2.0),
        hq_target_fps: args
            .hq_target_fps
            .unwrap_or(runtime_cfg.hq_target_fps)
            .clamp(12, 120),
        subject_exposure_only: args
            .subject_exposure_only
            .map(Into::into)
            .unwrap_or(runtime_cfg.subject_exposure_only),
        subject_target_height_ratio: args
            .subject_target_height
            .unwrap_or(runtime_cfg.subject_target_height_ratio)
            .clamp(0.20, 0.95),
        subject_target_width_ratio: args
            .subject_target_width
            .unwrap_or(runtime_cfg.subject_target_width_ratio)
            .clamp(0.10, 0.95),
        quality_auto_distance: args
            .quality_auto_distance
            .map(Into::into)
            .unwrap_or(runtime_cfg.quality_auto_distance),
        texture_mip_bias: args
            .texture_mip_bias
            .unwrap_or(runtime_cfg.texture_mip_bias)
            .clamp(-2.0, 4.0),
        stage_as_sub_only: args
            .stage_sub_only
            .map(Into::into)
            .unwrap_or(runtime_cfg.stage_as_sub_only),
        stage_role: args
            .stage_role
            .map(Into::into)
            .unwrap_or(runtime_cfg.stage_role),
        stage_quality: runtime_cfg.stage_quality,
        stage_luma_cap: args
            .stage_luma_cap
            .unwrap_or(runtime_cfg.stage_luma_cap)
            .clamp(0.0, 1.0),
        cell_aspect_mode: args
            .cell_aspect_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.cell_aspect_mode),
        cell_aspect_trim: args
            .cell_aspect_trim
            .unwrap_or(runtime_cfg.cell_aspect_trim)
            .clamp(0.70, 1.30),
        contrast_profile: args
            .contrast_profile
            .map(Into::into)
            .unwrap_or(runtime_cfg.contrast_profile),
        perf_profile: args
            .perf_profile
            .map(Into::into)
            .unwrap_or(runtime_cfg.perf_profile),
        detail_profile: args
            .detail_profile
            .map(Into::into)
            .unwrap_or(runtime_cfg.detail_profile),
        backend: args.backend.map(Into::into).unwrap_or(runtime_cfg.backend),
        exposure_bias: args
            .exposure_bias
            .unwrap_or(runtime_cfg.exposure_bias)
            .clamp(-0.5, 0.8),
        center_lock: args
            .center_lock
            .map(Into::into)
            .unwrap_or(runtime_cfg.center_lock),
        center_lock_mode: args
            .center_lock_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.center_lock_mode),
        wasd_mode: args
            .wasd_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.wasd_mode),
        freefly_speed: args
            .freefly_speed
            .unwrap_or(runtime_cfg.freefly_speed)
            .clamp(0.1, 8.0),
        camera_look_speed: args
            .camera_look_speed
            .unwrap_or(runtime_cfg.camera_look_speed)
            .clamp(0.1, 8.0),
        camera_mode: args
            .camera_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.camera_mode),
        camera_align_preset: args
            .camera_align_preset
            .map(Into::into)
            .unwrap_or(runtime_cfg.camera_align_preset),
        camera_unit_scale: args
            .camera_unit_scale
            .unwrap_or(runtime_cfg.camera_unit_scale)
            .clamp(0.01, 2.0),
        camera_vmd_fps: args
            .camera_vmd_fps
            .unwrap_or(runtime_cfg.camera_vmd_fps)
            .clamp(1.0, 240.0),
        camera_vmd_path: args
            .camera_vmd
            .clone()
            .or(runtime_cfg.camera_vmd_path.clone()),
        camera_focus: args
            .camera_focus
            .map(Into::into)
            .unwrap_or(runtime_cfg.camera_focus),
        material_color: args
            .material_color
            .map(Into::into)
            .unwrap_or(runtime_cfg.material_color),
        texture_sampling: args
            .texture_sampling
            .map(Into::into)
            .unwrap_or(runtime_cfg.texture_sampling),
        texture_v_origin: args
            .texture_v_origin
            .map(Into::into)
            .unwrap_or(runtime_cfg.texture_v_origin),
        texture_sampler: args
            .texture_sampler
            .map(Into::into)
            .unwrap_or(runtime_cfg.texture_sampler),
        clarity_profile: args
            .clarity_profile
            .map(Into::into)
            .unwrap_or(runtime_cfg.clarity_profile),
        ansi_quantization: args
            .ansi_quantization
            .map(Into::into)
            .unwrap_or(runtime_cfg.ansi_quantization),
        model_lift: args
            .model_lift
            .unwrap_or(runtime_cfg.model_lift)
            .clamp(0.02, 0.45),
        edge_accent_strength: args
            .edge_accent_strength
            .unwrap_or(runtime_cfg.edge_accent_strength)
            .clamp(0.0, 1.5),
        bg_suppression: runtime_cfg.bg_suppression.clamp(0.0, 1.0),
        braille_aspect_compensation: runtime_cfg.braille_aspect_compensation,
        stage_level: args.stage_level.unwrap_or(runtime_cfg.stage_level).min(4),
        stage_reactive: runtime_cfg.stage_reactive,
        color_mode: args.color_mode.map(Into::into).or(runtime_cfg.color_mode),
        ascii_force_color: args
            .ascii_force_color
            .map(Into::into)
            .unwrap_or(runtime_cfg.ascii_force_color),
        braille_profile: args
            .braille_profile
            .map(Into::into)
            .unwrap_or(runtime_cfg.braille_profile),
        theme_style: args
            .theme
            .map(Into::into)
            .unwrap_or(runtime_cfg.theme_style),
        audio_reactive: args
            .audio_reactive
            .map(Into::into)
            .unwrap_or(runtime_cfg.audio_reactive),
        cinematic_camera: args
            .cinematic_camera
            .map(Into::into)
            .unwrap_or(runtime_cfg.cinematic_camera),
        reactive_gain: args
            .reactive_gain
            .unwrap_or(runtime_cfg.reactive_gain)
            .clamp(0.0, 1.0),
    }
}
