use anyhow::{Result, bail};

use crate::{
    cli::StartArgs,
    interfaces::tui::start_ui::{StartWizardDefaults, run_start_wizard},
    runtime::{
        app::{load_runtime_config, resolve_animation_index},
        asset_discovery::{
            discover_camera_vmds, discover_glb_files, discover_music_files, discover_pmx_files,
            discover_stage_sets, discover_vmd_files, resolve_camera_vmd_choice,
            resolved_camera_dir, resolved_stage_dir,
        },
        options::{
            default_color_mode_for_mode, resolve_effective_color_mode,
            resolve_pmx_settings_for_start, resolve_sync_options_for_start,
            resolve_sync_profile_options_for_start, resolve_visual_options_for_start,
        },
        pmx_log,
        render_loop::run_scene_interactive,
    },
    scene::RenderMode,
};

mod font;
mod model;
mod render_config;
mod stage;
mod sync;

pub(super) fn start(args: StartArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_start(&args, &runtime_cfg);
    let sync_defaults = resolve_sync_options_for_start(&args, &runtime_cfg);
    let sync_profile_defaults = resolve_sync_profile_options_for_start(&args, &runtime_cfg);
    let pmx_settings = resolve_pmx_settings_for_start(&args, &runtime_cfg);
    let model_files = discover_glb_files(&args.dir)?;
    let pmx_files = discover_pmx_files(&args.pmx_dir)?;
    let motion_files = discover_vmd_files(&args.motion_dir);
    if model_files.is_empty() {
        bail!(
            "no .glb/.gltf files found in {}",
            args.dir.as_path().display()
        );
    }
    let music_files = discover_music_files(&args.music_dir)?;
    let stage_dir = resolved_stage_dir(&args.stage_dir, &runtime_cfg);
    let stage_entries = discover_stage_sets(&stage_dir);
    let camera_dir = resolved_camera_dir(&args.camera_dir, &runtime_cfg);
    let camera_files = discover_camera_vmds(&camera_dir);
    let runtime_camera_selector = runtime_cfg.camera_selection.as_str();
    let cli_camera_selector = args.camera.as_deref();
    let selector = cli_camera_selector.unwrap_or(runtime_camera_selector);
    let selector_explicit_none = selector.eq_ignore_ascii_case("none");
    let selected_camera_path = args
        .camera_vmd
        .clone()
        .or_else(|| resolve_camera_vmd_choice(&camera_dir, &camera_files, selector))
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                runtime_cfg.camera_vmd_path.clone()
            }
        });
    let start_mode: RenderMode = args.mode.into();
    let default_color_mode = resolve_effective_color_mode(
        start_mode,
        visual
            .color_mode
            .unwrap_or_else(|| default_color_mode_for_mode(start_mode)),
        visual.ascii_force_color,
    );
    let defaults = StartWizardDefaults {
        mode: start_mode,
        output_mode: visual.output_mode,
        graphics_protocol: visual.graphics_protocol,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        clarity_profile: visual.clarity_profile,
        ansi_quantization: visual.ansi_quantization,
        backend: visual.backend,
        center_lock: visual.center_lock,
        center_lock_mode: visual.center_lock_mode,
        wasd_mode: visual.wasd_mode,
        freefly_speed: visual.freefly_speed,
        camera_focus: visual.camera_focus,
        material_color: visual.material_color,
        texture_sampling: visual.texture_sampling,
        model_lift: visual.model_lift,
        edge_accent_strength: visual.edge_accent_strength,
        braille_aspect_compensation: visual.braille_aspect_compensation,
        stage_level: visual.stage_level,
        stage_reactive: visual.stage_reactive,
        color_mode: default_color_mode,
        braille_profile: visual.braille_profile,
        theme_style: visual.theme_style,
        audio_reactive: visual.audio_reactive,
        cinematic_camera: visual.cinematic_camera,
        reactive_gain: visual.reactive_gain,
        fps_cap: args.fps_cap,
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        contrast_profile: visual.contrast_profile,
        sync_offset_ms: sync_defaults.sync_offset_ms,
        sync_speed_mode: sync_defaults.sync_speed_mode,
        sync_policy: sync_defaults.sync_policy,
        sync_hard_snap_ms: sync_defaults.sync_hard_snap_ms,
        sync_kp: sync_defaults.sync_kp,
        font_preset_enabled: runtime_cfg.font_preset_enabled,
        camera_mode: visual.camera_mode,
        camera_align_preset: visual.camera_align_preset,
        camera_unit_scale: visual.camera_unit_scale,
        camera_vmd_path: selected_camera_path.clone(),
    };
    let Some(selection) = run_start_wizard(
        &args.dir,
        &args.pmx_dir,
        &args.motion_dir,
        &args.music_dir,
        &stage_dir,
        &camera_dir,
        &model_files,
        &pmx_files,
        &motion_files,
        &music_files,
        &camera_files,
        &stage_entries,
        defaults,
        runtime_cfg.ui_language,
        args.anim.as_deref(),
    )?
    else {
        return Ok(());
    };
    if selection.apply_font_preset {
        font::apply_startup_font_config(&runtime_cfg);
    }
    let mut scene = model::load_selected_model(&selection)?;
    scene = stage::apply_stage_selection(scene, selection.stage_choice.as_ref())?;
    let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
    if matches!(
        selection.branch,
        crate::interfaces::tui::start_ui::ModelBranch::PmxVmd
    ) {
        pmx_log::info(format!("resolved animation_index={animation_index:?}"));
        if animation_index.is_none() {
            pmx_log::warn("no animation clip was selected after PMX+VMD import.");
        }
    }
    let sync_result = sync::resolve_sync_and_audio(
        &selection,
        &scene,
        animation_index,
        &sync_profile_defaults,
        &sync_defaults,
        &args,
    );
    let render_build = render_config::build_render_config(
        &selection,
        &args,
        &visual,
        &sync_result.effective_sync,
        &runtime_cfg,
        start_mode,
    );
    run_scene_interactive(
        scene,
        animation_index,
        false,
        render_build.config,
        sync_result.audio_sync,
        sync_result.effective_sync.sync_offset_ms,
        args.orbit_speed,
        args.orbit_radius,
        args.camera_height,
        args.look_at_y,
        render_build.wasd_mode,
        render_build.freefly_speed,
        render_build.camera_settings,
        pmx_settings,
        sync_result.sync_profile_context,
    )
}
