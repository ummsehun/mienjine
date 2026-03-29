use anyhow::{bail, Context, Result};

use crate::{
    assets::vmd_motion::parse_vmd_motion,
    cli::{RunSceneArg, StartArgs},
    loader,
    render::backend_gpu::GpuRendererState,
    runtime::{
        app_render_config::render_config_from_start,
        asset_discovery::{
            apply_stage_transform, discover_camera_vmds, discover_glb_files, discover_music_files,
            discover_pmx_files, discover_stage_sets, discover_vmd_files, load_scene_file,
            merge_scenes, resolve_camera_vmd_choice, resolved_camera_dir, resolved_stage_dir,
        },
        audio_sync::prepare_audio_sync,
        config::GasciiConfig,
        interaction::max_scene_vertices,
        options::{
            default_color_mode_for_mode, resolve_effective_camera_mode,
            resolve_effective_color_mode, resolve_sync_options_for_start,
            resolve_sync_profile_for_assets, resolve_sync_profile_options_for_start,
            resolve_visual_options_for_start, ResolvedSyncOptions, ResolvedVisualOptions,
        },
        pmx_log,
        render_loop::run_scene_interactive,
        start_ui::{run_start_wizard, StageStatus, StartWizardDefaults},
        state::{resolve_runtime_backend, RuntimeCameraSettings},
    },
    scene::{resolve_cell_aspect, RenderMode},
};

use crate::runtime::app::{
    apply_runtime_render_tuning, load_runtime_config, resolve_animation_index,
};

pub(super) fn start(args: StartArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_start(&args, &runtime_cfg);
    let sync_defaults = resolve_sync_options_for_start(&args, &runtime_cfg);
    let sync_profile_defaults = resolve_sync_profile_options_for_start(&args, &runtime_cfg);
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
        apply_startup_font_config(&runtime_cfg);
    }
    let mut scene = match selection.branch {
        crate::runtime::start_ui::ModelBranch::Glb => loader::load_gltf(&selection.glb_path)?,
        crate::runtime::start_ui::ModelBranch::PmxVmd => {
            let pmx_path = selection
                .pmx_path
                .as_deref()
                .context("PMX branch selected without pmx_path")?;
            pmx_log::info("=== PMX+VMD import start ===");
            pmx_log::info(format!("PMX path: {}", pmx_path.display()));
            if let Some(motion_vmd_path) = selection.motion_vmd_path.as_deref() {
                pmx_log::info(format!("VMD path: {}", motion_vmd_path.display()));
            } else {
                pmx_log::warn("PMX branch selected without a VMD motion; model will load static.");
            }

            let mut scene = match loader::load_pmx(pmx_path) {
                Ok(scene) => scene,
                Err(err) => {
                    pmx_log::error(format!("failed to load PMX {}: {err}", pmx_path.display()));
                    return Err(err);
                }
            };
            pmx_log::info(format!(
                "PMX loaded: nodes={}, meshes={}, materials={}, material_morphs={}, ik_chains={}",
                scene.nodes.len(),
                scene.meshes.len(),
                scene.materials.len(),
                scene.material_morphs.len(),
                scene
                    .pmx_rig_meta
                    .as_ref()
                    .map(|meta| meta.ik_chains.len())
                    .unwrap_or(0)
            ));
            if let Some(motion_vmd_path) = selection.motion_vmd_path.as_deref() {
                match parse_vmd_motion(motion_vmd_path) {
                    Ok(vmd) => {
                        pmx_log::info(format!(
                            "VMD parsed: model_name='{}', bone_frames={}, morph_frames={}, duration={:.3}s",
                            vmd.model_name,
                            vmd.bone_frames.len(),
                            vmd.morph_frames.len(),
                            vmd.duration_secs()
                        ));
                        if !vmd.bone_frames.is_empty() || !vmd.morph_frames.is_empty() {
                            let clip = vmd.to_clip_for_scene(&scene);
                            pmx_log::info(format!(
                                "VMD clip built: channels={}, duration={:.3}s",
                                clip.channels.len(),
                                clip.duration
                            ));
                            if clip.channels.is_empty() {
                                pmx_log::warn(
                                    "VMD clip has no matched channels; bone/morph names may not match this PMX.",
                                );
                            }
                            scene.animations.push(clip);
                        } else {
                            pmx_log::warn(format!(
                                "VMD {} contains no bone or morph frames.",
                                motion_vmd_path.display()
                            ));
                        }
                    }
                    Err(err) => {
                        pmx_log::error(format!(
                            "failed to parse VMD {}: {err}",
                            motion_vmd_path.display()
                        ));
                    }
                }
            }
            pmx_log::info(format!(
                "PMX+VMD scene animations={}",
                scene.animations.len()
            ));
            scene
        }
    };
    if let Some(stage_choice) = selection.stage_choice.as_ref() {
        match stage_choice.status {
            StageStatus::Ready => {
                if let Some(stage_path) = stage_choice.render_path.as_deref() {
                    match load_scene_file(stage_path) {
                        Ok(mut stage_scene) => {
                            apply_stage_transform(&mut stage_scene, stage_choice.transform);
                            scene = merge_scenes(scene, stage_scene);
                        }
                        Err(err) => {
                            eprintln!(
                                "warning: failed to load stage {}: {err}",
                                stage_path.display()
                            );
                        }
                    }
                }
            }
            StageStatus::NeedsConvert => {
                let pmx = stage_choice
                    .pmx_path
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| stage_choice.name.clone());
                bail!(
                    "선택한 스테이지는 PMX 변환이 필요합니다: {pmx}\nBlender + MMD Tools로 GLB 변환 후 다시 실행하세요."
                );
            }
            StageStatus::Invalid => {
                eprintln!(
                    "warning: selected stage '{}' is invalid (no renderable assets). continuing without stage.",
                    stage_choice.name
                );
            }
        }
    }
    let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
    if matches!(
        selection.branch,
        crate::runtime::start_ui::ModelBranch::PmxVmd
    ) {
        pmx_log::info(format!("resolved animation_index={animation_index:?}"));
        if animation_index.is_none() {
            pmx_log::warn("no animation clip was selected after PMX+VMD import.");
        }
    }
    let (sync_profile_context, sync_profile_entry) = resolve_sync_profile_for_assets(
        &sync_profile_defaults,
        match selection.branch {
            crate::runtime::start_ui::ModelBranch::Glb => RunSceneArg::Glb,
            crate::runtime::start_ui::ModelBranch::PmxVmd => RunSceneArg::Pmx,
        },
        Some(match selection.branch {
            crate::runtime::start_ui::ModelBranch::Glb => selection.glb_path.as_path(),
            crate::runtime::start_ui::ModelBranch::PmxVmd => selection
                .pmx_path
                .as_deref()
                .unwrap_or(selection.glb_path.as_path()),
        }),
        selection.music_path.as_deref(),
        selection.camera_vmd_path.as_deref(),
    );
    let mut effective_sync = ResolvedSyncOptions {
        sync_offset_ms: selection.sync_offset_ms,
        sync_speed_mode: selection.sync_speed_mode,
        sync_policy: selection.sync_policy,
        sync_hard_snap_ms: selection.sync_hard_snap_ms,
        sync_kp: selection.sync_kp,
    };
    if let Some(profile) = sync_profile_entry.as_ref() {
        if args.sync_offset_ms.is_none() && selection.sync_offset_ms == sync_defaults.sync_offset_ms
        {
            effective_sync.sync_offset_ms = profile.sync_offset_ms;
        }
        if args.sync_speed_mode.is_none()
            && selection.sync_speed_mode == sync_defaults.sync_speed_mode
            && profile.sync_speed_mode.is_some()
        {
            effective_sync.sync_speed_mode = profile
                .sync_speed_mode
                .unwrap_or(sync_defaults.sync_speed_mode);
        }
        if args.sync_hard_snap_ms.is_none()
            && selection.sync_hard_snap_ms == sync_defaults.sync_hard_snap_ms
            && profile.sync_hard_snap_ms.is_some()
        {
            effective_sync.sync_hard_snap_ms = profile
                .sync_hard_snap_ms
                .unwrap_or(sync_defaults.sync_hard_snap_ms)
                .clamp(10, 2_000);
        }
        if args.sync_kp.is_none()
            && selection.sync_kp == sync_defaults.sync_kp
            && profile.sync_kp.is_some()
        {
            effective_sync.sync_kp = profile
                .sync_kp
                .unwrap_or(sync_defaults.sync_kp)
                .clamp(0.01, 1.0);
        }
    }
    let clip_duration_secs = animation_index
        .and_then(|idx| scene.animations.get(idx))
        .map(|clip| clip.duration);
    let audio_sync = prepare_audio_sync(
        selection.music_path.as_deref(),
        clip_duration_secs,
        effective_sync.sync_speed_mode,
    );
    if selection.music_path.is_some() && audio_sync.is_none() {
        eprintln!("warning: audio playback unavailable. continuing in silent mode.");
    }
    let mut config = render_config_from_start(
        &args,
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
    apply_runtime_render_tuning(&mut config, &runtime_cfg);
    run_scene_interactive(
        scene,
        animation_index,
        false,
        config,
        audio_sync,
        effective_sync.sync_offset_ms,
        args.orbit_speed,
        args.orbit_radius,
        args.camera_height,
        args.look_at_y,
        wasd_mode,
        freefly_speed,
        camera_settings,
        sync_profile_context,
    )
}

fn apply_startup_font_config(runtime_cfg: &GasciiConfig) {
    if runtime_cfg.font_preset_enabled {
        run_ghostty_font_shortcut("0");
    }
    let steps = runtime_cfg.font_preset_steps;
    if steps > 0 {
        for _ in 0..steps {
            run_ghostty_font_shortcut("=");
        }
    } else if steps < 0 {
        for _ in 0..(-steps) {
            run_ghostty_font_shortcut("-");
        }
    }
}

pub(super) fn run_ghostty_font_shortcut(key: &str) {
    if !crate::terminal::TerminalProfile::detect().is_ghostty {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Ghostty\" to activate\ntell application \"System Events\" to keystroke \"{}\" using command down",
            key
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = key;
    }
}
