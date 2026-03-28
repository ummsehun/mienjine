use std::{
    panic,
    path::Path,
    sync::{Mutex, Once, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};

use crate::{
    animation::ChannelTarget,
    cli::{
        BenchArgs, BenchSceneArg, Cli, Commands, PreprocessArgs, PreviewArgs, RunArgs, RunSceneArg,
    },
    loader,
    pipeline::FramePipeline,
    render::backend::render_frame_with_backend,
    renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch},
    runtime::{
        app_inspect::inspect,
        app_render_config::{render_config_for_bench, render_config_from_run},
        app_start::start,
        asset_discovery::{
            apply_stage_transform, discover_camera_vmds, discover_stage_sets, load_scene_file,
            merge_scenes, resolve_camera_vmd_choice, resolve_stage_choice_from_selector,
            resolved_camera_dir, resolved_stage_dir, resolved_stage_selector,
        },
        config::{load_gascii_config, GasciiConfig},
        graphics_proto::{cleanup_orphan_shm_files, cleanup_shm_registry},
        interaction::max_scene_vertices,
        options::{
            resolve_effective_camera_mode, resolve_sync_options_for_run,
            resolve_sync_profile_for_assets, resolve_sync_profile_options_for_run,
            resolve_visual_options_for_bench, resolve_visual_options_for_run,
            RuntimeSyncProfileContext,
        },
        preprocess::run_preprocess,
        preview::run_preview_server,
        render_loop::run_scene_interactive,
        start_ui::StageStatus,
        state::{resolve_runtime_backend, RuntimeCameraSettings},
        sync_profile::{
            build_profile_key, default_profile_store_path, SyncProfileEntry, SyncProfileMode,
            SyncProfileStore,
        },
    },
    scene::{resolve_cell_aspect, CellAspectMode, RenderConfig, SceneCpu},
};

static PANIC_HOOK_ONCE: Once = Once::new();
static LAST_RUNTIME_STATE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn run(cli: Cli) -> Result<()> {
    install_runtime_panic_hook_once();
    let cleaned = cleanup_orphan_shm_files();
    if cleaned > 0 {
        eprintln!("info: cleaned {cleaned} orphan kitty shm buffer(s)");
    }
    match cli.command {
        Commands::Start(args) => start(args),
        Commands::Run(args) => run_interactive(args),
        Commands::Preview(args) => preview(args),
        Commands::Preprocess(args) => preprocess(args),
        Commands::Bench(args) => bench(args),
        Commands::Inspect(args) => inspect(args),
    }
}

fn run_interactive(args: RunArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_run(&args, &runtime_cfg);
    let sync_profile_defaults = resolve_sync_profile_options_for_run(&args, &runtime_cfg);
    let camera_dir = resolved_camera_dir(&args.camera_dir, &runtime_cfg);
    let camera_files = discover_camera_vmds(&camera_dir);
    let camera_selector = args
        .camera
        .as_deref()
        .unwrap_or(&runtime_cfg.camera_selection);
    let selector_explicit_none = camera_selector.eq_ignore_ascii_case("none");
    let resolved_camera_vmd_path = args
        .camera_vmd
        .clone()
        .or_else(|| resolve_camera_vmd_choice(&camera_dir, &camera_files, camera_selector))
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                visual.camera_vmd_path.clone()
            }
        });
    let (sync_profile_context, sync_profile_entry) = resolve_sync_profile_for_assets(
        &sync_profile_defaults,
        args.scene,
        if matches!(args.scene, RunSceneArg::Glb) {
            args.glb.as_deref()
        } else {
            None
        },
        None,
        resolved_camera_vmd_path.as_deref(),
    );
    let sync = resolve_sync_options_for_run(&args, &runtime_cfg, sync_profile_entry.as_ref());
    let (mut scene, animation_index, rotates_without_animation) = load_scene_for_run(&args)?;
    let stage_dir = resolved_stage_dir(&args.stage_dir, &runtime_cfg);
    let stage_selector = resolved_stage_selector(args.stage.as_deref(), &runtime_cfg);
    let stage_entries = discover_stage_sets(&stage_dir);
    if let Some(stage_choice) = resolve_stage_choice_from_selector(&stage_entries, &stage_selector)
    {
        match stage_choice.status {
            StageStatus::Ready => {
                if let Some(path) = stage_choice.render_path.as_deref() {
                    match load_scene_file(path) {
                        Ok(mut stage_scene) => {
                            apply_stage_transform(&mut stage_scene, stage_choice.transform);
                            scene = merge_scenes(scene, stage_scene);
                        }
                        Err(err) => {
                            eprintln!("warning: failed to load stage {}: {err}", path.display());
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
                    "selected stage requires PMX conversion before runtime: {pmx}\nConvert to GLB and retry."
                );
            }
            StageStatus::Invalid => {
                eprintln!(
                    "warning: selected stage '{}' is invalid. running without stage.",
                    stage_choice.name
                );
            }
        }
    }
    let mut config = render_config_from_run(&args, &visual);
    config.sync_policy = sync.sync_policy;
    config.sync_hard_snap_ms = sync.sync_hard_snap_ms;
    config.sync_kp = sync.sync_kp;
    apply_runtime_render_tuning(&mut config, &runtime_cfg);
    let effective_camera_mode =
        resolve_effective_camera_mode(visual.camera_mode, resolved_camera_vmd_path.is_some());
    let camera_settings = RuntimeCameraSettings {
        mode: effective_camera_mode,
        align_preset: visual.camera_align_preset,
        unit_scale: visual.camera_unit_scale,
        vmd_fps: visual.camera_vmd_fps,
        vmd_path: resolved_camera_vmd_path.clone(),
        look_speed: visual.camera_look_speed,
    };
    run_scene_interactive(
        scene,
        animation_index,
        rotates_without_animation,
        config,
        None,
        sync.sync_offset_ms,
        args.orbit_speed,
        args.orbit_radius,
        args.camera_height,
        args.look_at_y,
        visual.wasd_mode,
        visual.freefly_speed,
        camera_settings,
        sync_profile_context,
    )
}

fn preview(args: PreviewArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let camera_dir = runtime_cfg.camera_dir.clone();
    let camera_files = discover_camera_vmds(&camera_dir);
    let selector_explicit_none = runtime_cfg.camera_selection.eq_ignore_ascii_case("none");
    let camera_path = args
        .camera_vmd
        .clone()
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                runtime_cfg.camera_vmd_path.clone()
            }
        })
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                resolve_camera_vmd_choice(&camera_dir, &camera_files, &runtime_cfg.camera_selection)
            }
        });
    let profile_key = build_profile_key(
        "glb",
        Some(args.glb.as_path()),
        None,
        camera_path.as_deref(),
    );
    let (profile_hit, resolved_offset) =
        if matches!(runtime_cfg.sync_profile_mode, SyncProfileMode::Off) {
            (false, runtime_cfg.sync_offset_ms)
        } else {
            let store_path = default_profile_store_path(&runtime_cfg.sync_profile_dir);
            match SyncProfileStore::load(&store_path) {
                Ok(store) => match store.get(&profile_key) {
                    Some(entry) => (true, entry.sync_offset_ms),
                    None => (false, runtime_cfg.sync_offset_ms),
                },
                Err(err) => {
                    eprintln!(
                        "warning: preview sync profile load failed {}: {err}",
                        store_path.display()
                    );
                    (false, runtime_cfg.sync_offset_ms)
                }
            }
        };
    run_preview_server(
        &args,
        camera_path,
        resolved_offset,
        if matches!(runtime_cfg.sync_profile_mode, SyncProfileMode::Off) {
            None
        } else {
            Some(profile_key)
        },
        profile_hit,
    )
}

fn preprocess(args: PreprocessArgs) -> Result<()> {
    run_preprocess(&args)
}

fn load_runtime_config() -> GasciiConfig {
    load_gascii_config(Path::new("Gascii.config"))
}

fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: &GasciiConfig) {
    config.triangle_stride = runtime_cfg.triangle_stride.max(1);
    config.min_triangle_area_px2 = runtime_cfg.min_triangle_area_px2.max(0.0);
    config.braille_aspect_compensation = runtime_cfg.braille_aspect_compensation;
}

pub(crate) fn persist_sync_profile_offset(
    context: &RuntimeSyncProfileContext,
    sync_offset_ms: i32,
) -> Result<()> {
    let mut store = SyncProfileStore::load(&context.store_path)?;
    let mut merged = SyncProfileEntry::with_offset(sync_offset_ms.clamp(-5_000, 5_000));
    if let Some(existing) = store.get(&context.key) {
        merged.sync_hard_snap_ms = existing.sync_hard_snap_ms;
        merged.sync_kp = existing.sync_kp;
        merged.sync_speed_mode = existing.sync_speed_mode;
    }
    store.upsert(context.key.clone(), merged);
    store.save_atomic(&context.store_path)
}

pub(crate) fn set_runtime_panic_state(line: String) {
    let lock = LAST_RUNTIME_STATE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut guard) = lock.lock() {
        *guard = line;
    }
}

fn install_runtime_panic_hook_once() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            cleanup_shm_registry();
            if let Some(lock) = LAST_RUNTIME_STATE.get() {
                if let Ok(state) = lock.lock() {
                    eprintln!("panic_state: {}", state.as_str());
                }
            }
            default_hook(panic_info);
        }));
    });
}

fn load_scene_for_run(args: &RunArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
    match args.scene {
        RunSceneArg::Cube => Ok((crate::scene::cube_scene(), None, true)),
        RunSceneArg::Obj => {
            let path = required_path(args.obj.as_deref(), "--obj is required for --scene obj")?;
            Ok((loader::load_obj(path)?, None, true))
        }
        RunSceneArg::Glb => {
            let path = required_path(args.glb.as_deref(), "--glb is required for --scene glb")?;
            let scene = loader::load_gltf(path)?;
            let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
            Ok((scene, animation_index, true))
        }
        RunSceneArg::Pmx => {
            let path = required_path(args.pmx.as_deref(), "--pmx is required for --scene pmx")?;
            let scene = loader::load_pmx(path)?;
            Ok((scene, None, true))
        }
    }
}

fn bench(args: BenchArgs) -> Result<()> {
    let (scene, animation_index, rotates) = load_scene_for_bench(&args)?;
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_bench(&args, &runtime_cfg);
    let mut config = render_config_for_bench(
        args.mode.into(),
        args.fov_deg,
        args.near,
        args.far,
        args.charset,
        args.cell_aspect,
        args.ambient,
        args.diffuse_strength,
        args.specular_strength,
        args.specular_power,
        args.rim_strength,
        args.rim_power,
        args.fog_strength,
        runtime_cfg.sync_policy,
        runtime_cfg.sync_hard_snap_ms,
        runtime_cfg.sync_kp,
        &visual,
    );
    apply_runtime_render_tuning(&mut config, &runtime_cfg);
    config.backend = resolve_runtime_backend(config.backend);
    config.cell_aspect = resolve_cell_aspect(&config, None);
    config.cell_aspect_mode = CellAspectMode::Manual;
    let mut frame = FrameBuffers::new(args.width.max(1), args.height.max(1));
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let camera = Camera::default();
    let mut gpu_renderer_state = crate::render::backend_gpu::GpuRendererState::default();

    let benchmark_duration = Duration::from_secs_f32(args.seconds.max(0.1));
    let started = Instant::now();
    let mut frames: u64 = 0;
    let mut triangles: u64 = 0;
    let mut pixels: u64 = 0;

    while started.elapsed() < benchmark_duration {
        let elapsed = started.elapsed().as_secs_f32();
        pipeline.prepare_frame(&scene, elapsed, animation_index);
        let stats = render_frame_with_backend(
            &mut gpu_renderer_state,
            &mut frame,
            &config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            pipeline.morph_weights_by_instance(),
            pipeline.material_morph_weights(),
            &glyph_ramp,
            &mut render_scratch,
            camera,
            if rotates { elapsed * 0.9 } else { 0.0 },
        );
        frames += 1;
        triangles += stats.triangles_total as u64;
        pixels += stats.pixels_drawn as u64;
    }

    let elapsed = started.elapsed().as_secs_f64();
    let fps = (frames as f64) / elapsed;
    println!("scene: {:?}", args.scene);
    println!("seconds: {:.2}", elapsed);
    println!("frames: {}", frames);
    println!("fps: {:.2}", fps);
    println!(
        "avg_triangles_per_frame: {:.2}",
        triangles as f64 / (frames.max(1) as f64)
    );
    println!(
        "avg_pixels_per_frame: {:.2}",
        pixels as f64 / (frames.max(1) as f64)
    );
    Ok(())
}

fn resolve_animation_index(scene: &SceneCpu, selector: Option<&str>) -> Result<Option<usize>> {
    if let Some(selector) = selector {
        let index = scene
            .animation_index_by_selector(Some(selector))
            .with_context(|| format!("animation selector not found: {selector}"))?;
        return Ok(Some(index));
    }
    Ok(default_body_animation_index(scene))
}

fn default_body_animation_index(scene: &SceneCpu) -> Option<usize> {
    scene
        .animations
        .iter()
        .enumerate()
        .find(|(_, clip)| {
            !clip.channels.is_empty()
                && clip
                    .channels
                    .iter()
                    .any(|channel| channel.target != ChannelTarget::MorphWeights)
        })
        .map(|(index, _)| index)
        .or_else(|| (!scene.animations.is_empty()).then_some(0))
}

fn load_scene_for_bench(args: &BenchArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
    match args.scene {
        BenchSceneArg::Cube => Ok((crate::scene::cube_scene(), None, true)),
        BenchSceneArg::Obj => {
            let path = required_path(args.obj.as_deref(), "--obj is required for --scene obj")?;
            Ok((loader::load_obj(path)?, None, true))
        }
        BenchSceneArg::GlbStatic => {
            let path = required_path(
                args.glb.as_deref(),
                "--glb is required for --scene glb-static",
            )?;
            Ok((loader::load_gltf(path)?, None, false))
        }
        BenchSceneArg::GlbAnim => {
            let path = required_path(
                args.glb.as_deref(),
                "--glb is required for --scene glb-anim",
            )?;
            let scene = loader::load_gltf(path)?;
            let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
            if animation_index.is_none() {
                bail!("scene has no animation clips: {}", path.display());
            }
            Ok((scene, animation_index, false))
        }
    }
}

fn required_path<'a>(path: Option<&'a Path>, message: &str) -> Result<&'a Path> {
    path.ok_or_else(|| anyhow::anyhow!("{message}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::{
        renderer::Camera,
        renderer::RenderStats,
        runtime::{
            asset_discovery,
            audio_sync::{compute_animation_speed_factor, compute_animation_time},
            interaction::update_camera_director,
            scene_analysis::compute_scene_framing,
            start_ui::{StageChoice, StageTransform},
            state::{
                apply_distant_subject_clarity_boost, apply_pmx_surface_guardrails, cap_render_size,
                dynamic_clip_planes, is_terminal_size_unstable, CameraDirectorState,
                CenterLockState, ContinuousSyncState, DistanceClampGuard, ExposureAutoBoost,
                OrbitState, RuntimeAdaptiveQuality, RuntimeCameraState, ScreenFitController,
                LOW_VIS_EXPOSURE_RECOVER_FRAMES, LOW_VIS_EXPOSURE_TRIGGER_FRAMES, MAX_RENDER_COLS,
                MAX_RENDER_ROWS,
            },
        },
        scene::{
            CameraControlMode, CameraFocusMode, CameraMode, CenterLockMode, CinematicCameraMode,
            ColorMode, PerfProfile, SyncPolicy, SyncSpeedMode,
        },
    };
    use glam::{Quat, Vec3};
    use tempfile::tempdir;

    #[test]
    fn auto_speed_factor_matches_reference_ratio() {
        let factor = compute_animation_speed_factor(
            Some(174.10),
            Some(170.480_907),
            SyncSpeedMode::AutoDurationFit,
        );
        assert!((factor - 1.021_229).abs() < 1e-4);
    }

    #[test]
    fn auto_speed_factor_allows_large_duration_ratio() {
        let factor = compute_animation_speed_factor(
            Some(300.0),
            Some(120.0),
            SyncSpeedMode::AutoDurationFit,
        );
        assert!((factor - 2.5).abs() < 1e-6);
    }

    #[test]
    fn animation_time_applies_sync_offset_with_audio_clock() {
        let mut state = ContinuousSyncState::default();
        let time = compute_animation_time(
            &mut state,
            SyncPolicy::Fixed,
            0.016,
            5.0,
            Some(3.0),
            1.05,
            120,
            120,
            0.15,
            None,
        );
        assert!((time - 3.27).abs() < 1e-6);
    }

    #[test]
    fn continuous_sync_tracks_drift_ema_and_hard_snaps() {
        let mut state = ContinuousSyncState::default();
        // First sample initializes near target.
        let _ = compute_animation_time(
            &mut state,
            SyncPolicy::Continuous,
            0.016,
            0.016,
            Some(0.0),
            1.0,
            0,
            120,
            0.15,
            None,
        );
        // Large target jump should trigger a hard snap and non-zero drift metric.
        let _ = compute_animation_time(
            &mut state,
            SyncPolicy::Continuous,
            0.016,
            0.032,
            Some(2.0),
            1.0,
            0,
            120,
            0.15,
            None,
        );
        assert!(state.drift_ema > 0.0);
        assert!(state.hard_snap_count >= 1);
    }

    fn simulate_continuous_sync(
        clip_duration: f32,
        audio_duration: f32,
        total_seconds: f32,
    ) -> (f32, u32, f32) {
        let dt = 1.0 / 60.0;
        let warmup = 10.0;
        let mut elapsed_wall = 0.0_f32;
        let mut max_err_after_warmup = 0.0_f32;
        let mut state = ContinuousSyncState::default();
        let speed_factor = compute_animation_speed_factor(
            Some(clip_duration),
            Some(audio_duration),
            SyncSpeedMode::AutoDurationFit,
        );

        while elapsed_wall < total_seconds {
            elapsed_wall += dt;
            let elapsed_audio = elapsed_wall.rem_euclid(audio_duration);
            let anim_time = compute_animation_time(
                &mut state,
                SyncPolicy::Continuous,
                dt,
                elapsed_wall,
                Some(elapsed_audio),
                speed_factor,
                0,
                120,
                0.15,
                Some(clip_duration),
            );
            let target = elapsed_audio * speed_factor;
            let raw = (target - anim_time).abs();
            let err = raw.min((clip_duration - raw).abs());
            if elapsed_wall >= warmup {
                max_err_after_warmup = max_err_after_warmup.max(err);
            }
        }

        (max_err_after_warmup, state.hard_snap_count, state.drift_ema)
    }

    #[test]
    fn continuous_sync_converges_when_clip_longer_than_audio() {
        let (max_err, hard_snaps, drift_ema) = simulate_continuous_sync(120.0, 117.0, 180.0);
        assert!(max_err <= 0.120);
        assert!(hard_snaps <= 9);
        assert!(drift_ema.is_finite());
    }

    #[test]
    fn continuous_sync_converges_when_audio_longer_than_clip() {
        let (max_err, hard_snaps, drift_ema) = simulate_continuous_sync(117.0, 120.0, 180.0);
        assert!(max_err <= 0.120);
        assert!(hard_snaps <= 9);
        assert!(drift_ema.is_finite());
    }

    #[test]
    fn auto_framing_focus_y_uses_center() {
        let scene = crate::scene::cube_scene();
        let framing = compute_scene_framing(&scene, RenderConfig::default().fov_deg, 0.0, 0.0, 0.0);
        assert!(framing.focus.y.abs() < 0.05);
    }

    #[test]
    fn mode_defaults_to_expected_color_mode() {
        assert!(matches!(
            default_color_mode_for_mode(RenderMode::Ascii),
            ColorMode::Mono
        ));
        assert!(matches!(
            default_color_mode_for_mode(RenderMode::Braille),
            ColorMode::Ansi
        ));
    }

    #[test]
    fn ascii_force_color_overrides_requested_mono() {
        assert!(matches!(
            resolve_effective_color_mode(RenderMode::Ascii, ColorMode::Mono, true),
            ColorMode::Ansi
        ));
        assert!(matches!(
            resolve_effective_color_mode(RenderMode::Braille, ColorMode::Mono, true),
            ColorMode::Mono
        ));
    }

    #[test]
    fn camera_mode_is_promoted_when_vmd_source_exists() {
        assert!(matches!(
            resolve_effective_camera_mode(CameraMode::Off, true),
            CameraMode::Vmd
        ));
        assert!(matches!(
            resolve_effective_camera_mode(CameraMode::Blend, true),
            CameraMode::Blend
        ));
        assert!(matches!(
            resolve_effective_camera_mode(CameraMode::Off, false),
            CameraMode::Off
        ));
    }

    #[test]
    fn default_animation_prefers_non_morph_clip() {
        use crate::animation::{
            AnimationChannel, AnimationClip, ChannelTarget, ChannelValues, Interpolation,
        };
        use crate::scene::{MeshCpu, MeshInstance, MeshLayer, MorphTargetCpu, Node, SceneCpu};
        use glam::Vec3;

        let scene = SceneCpu {
            meshes: vec![MeshCpu {
                positions: vec![Vec3::ZERO],
                normals: vec![Vec3::Y],
                uv0: None,
                uv1: None,
                colors_rgba: None,
                material_index: None,
                indices: vec![[0, 0, 0]],
                joints4: None,
                weights4: None,
                morph_targets: vec![MorphTargetCpu {
                    name: Some("move_up".to_owned()),
                    position_deltas: vec![Vec3::new(0.0, 1.0, 0.0)],
                    normal_deltas: vec![Vec3::ZERO],
                }],
            }],
            materials: Vec::new(),
            textures: Vec::new(),
            skins: Vec::new(),
            nodes: vec![Node {
                name: Some("root".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::ZERO,
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            }],
            mesh_instances: vec![MeshInstance {
                mesh_index: 0,
                node_index: 0,
                skin_index: None,
                default_morph_weights: vec![0.0],
                layer: MeshLayer::Subject,
            }],
            animations: vec![
                AnimationClip {
                    name: Some("face".to_owned()),
                    channels: vec![AnimationChannel {
                        node_index: 0,
                        target: ChannelTarget::MorphWeights,
                        interpolation: Interpolation::Linear,
                        inputs: vec![0.0, 1.0],
                        outputs: ChannelValues::MorphWeights {
                            values: vec![0.0, 1.0],
                            weights_per_key: 1,
                        },
                    }],
                    duration: 1.0,
                    looping: true,
                },
                AnimationClip {
                    name: Some("body".to_owned()),
                    channels: vec![AnimationChannel {
                        node_index: 0,
                        target: ChannelTarget::Translation,
                        interpolation: Interpolation::Linear,
                        inputs: vec![0.0, 1.0],
                        outputs: ChannelValues::Vec3(vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)]),
                    }],
                    duration: 1.0,
                    looping: true,
                },
            ],
            root_center_node: Some(0),
            pmx_rig_meta: None,
            material_morphs: Vec::new(),
        };

        assert_eq!(default_body_animation_index(&scene), Some(1));
    }

    #[test]
    fn runtime_camera_starts_in_orbit_when_track_is_available() {
        let state = RuntimeCameraState::new(CameraControlMode::FreeFly, CameraMode::Vmd, true);
        assert!(matches!(state.control_mode, CameraControlMode::Orbit));
        assert!(state.track_enabled);
    }

    #[test]
    fn distant_subject_clarity_boost_strengthens_subject_visibility() {
        let mut cfg = RenderConfig::default();
        cfg.model_lift = 0.10;
        cfg.edge_accent_strength = 0.20;
        cfg.bg_suppression = 0.20;
        cfg.triangle_stride = 3;
        cfg.min_triangle_area_px2 = 0.8;
        apply_distant_subject_clarity_boost(&mut cfg, 0.10);
        assert!(cfg.model_lift > 0.10);
        assert!(cfg.edge_accent_strength > 0.20);
        assert!(cfg.bg_suppression > 0.20);
        assert!(cfg.triangle_stride < 3);
        assert!(cfg.min_triangle_area_px2 < 0.8);
    }

    #[test]
    fn pmx_surface_guardrails_clamp_sparse_rendering_on_small_subjects() {
        let mut cfg = RenderConfig::default();
        cfg.triangle_stride = 3;
        cfg.min_triangle_area_px2 = 0.8;
        cfg.edge_accent_strength = 0.9;

        apply_pmx_surface_guardrails(&mut cfg, true, 0.20);

        assert_eq!(cfg.triangle_stride, 1);
        assert!(cfg.min_triangle_area_px2 <= 0.12);
        assert!(cfg.edge_accent_strength <= 0.26);
    }

    #[test]
    fn center_lock_camera_space_moves_camera_when_anchor_is_offcenter() {
        let mut state = CenterLockState::default();
        let mut stats = RenderStats::default();
        stats.subject_centroid_px = Some((10.0, 20.0));
        let mut camera = Camera::default();
        let before = camera.eye;
        state.apply_camera_space(
            &stats,
            CenterLockMode::Root,
            120,
            40,
            &mut camera,
            60.0,
            0.5,
            2.0,
        );
        assert!((camera.eye - before).length() > 1e-6);
    }

    #[test]
    fn screen_fit_controller_uses_mode_specific_targets() {
        let mut controller = ScreenFitController::default();
        controller.update(0.40, RenderMode::Ascii, true);
        let ascii_gain = controller.auto_zoom_gain;
        assert!(ascii_gain > 1.0);

        controller = ScreenFitController::default();
        controller.update(0.40, RenderMode::Braille, true);
        let braille_gain = controller.auto_zoom_gain;
        assert!(braille_gain > 1.0);
        assert!(ascii_gain >= braille_gain);
    }

    #[test]
    fn exposure_auto_boost_ramps_and_recovers() {
        let mut boost = ExposureAutoBoost::default();
        for _ in 0..LOW_VIS_EXPOSURE_TRIGGER_FRAMES {
            boost.update(0.001);
        }
        assert!(boost.boost > 0.0);
        let boosted = boost.boost;
        for _ in 0..LOW_VIS_EXPOSURE_RECOVER_FRAMES {
            boost.update(0.05);
        }
        assert!(boost.boost < boosted);
    }

    #[test]
    fn camera_director_outputs_stable_values() {
        let mut director = CameraDirectorState::default();
        let (radius, height, focus_y, jitter) = update_camera_director(
            &mut director,
            CinematicCameraMode::On,
            CameraFocusMode::Auto,
            0.1,
            0.6,
            0.35,
            1.2,
            1.0,
        );
        assert!(radius > 0.0);
        assert!(height.abs() < 1.0);
        assert!(focus_y.abs() < 1.0);
        assert!(jitter.abs() <= 0.015 + 1e-3);
    }

    #[test]
    fn orbit_state_holds_angle_when_disabled() {
        let mut orbit = OrbitState::new(0.0);
        orbit.angle = 1.23;
        orbit.advance(1.0);
        assert!((orbit.angle - 1.23).abs() < 1e-6);
    }

    #[test]
    fn adaptive_quality_moves_lod_on_thresholds() {
        let mut quality = RuntimeAdaptiveQuality::new(PerfProfile::Balanced);
        for _ in 0..30 {
            quality.observe(90.0);
        }
        assert!(quality.lod_level >= 1);

        for _ in 0..90 {
            quality.observe(8.0);
        }
        assert!(quality.lod_level <= 1);
    }

    #[test]
    fn cap_render_size_applies_upper_bound() {
        let (w, h, scaled) = cap_render_size(6000, 3200);
        assert!(scaled);
        assert!(w <= MAX_RENDER_COLS);
        assert!(h <= MAX_RENDER_ROWS);
    }

    #[test]
    fn terminal_size_unstable_only_for_invalid_or_sentinel_values() {
        assert!(is_terminal_size_unstable(0, 40));
        assert!(is_terminal_size_unstable(120, 0));
        assert!(is_terminal_size_unstable(u16::MAX, 40));
        assert!(is_terminal_size_unstable(120, u16::MAX));
        assert!(!is_terminal_size_unstable(432, 102));
        assert!(!is_terminal_size_unstable(900, 140));
    }

    #[test]
    fn discover_stage_sets_classifies_ready_and_convert() {
        let dir = tempdir().expect("tempdir");
        let stage_root = dir.path().join("assets").join("stage");
        let ready_dir = stage_root.join("ready_stage");
        let convert_dir = stage_root.join("pmx_stage");
        let invalid_dir = stage_root.join("empty_stage");
        fs::create_dir_all(&ready_dir).expect("ready dir");
        fs::create_dir_all(&convert_dir).expect("convert dir");
        fs::create_dir_all(&invalid_dir).expect("invalid dir");
        fs::write(ready_dir.join("scene.glb"), b"not-a-real-glb").expect("ready file");
        fs::write(convert_dir.join("stage.pmx"), b"pmx").expect("pmx file");

        let stages = discover_stage_sets(&stage_root);
        assert_eq!(stages.len(), 3);
        assert!(stages.iter().any(|s| {
            s.name == "ready_stage"
                && matches!(s.status, StageStatus::Ready)
                && s.render_path.is_some()
        }));
        assert!(stages.iter().any(|s| {
            s.name == "pmx_stage"
                && matches!(s.status, StageStatus::NeedsConvert)
                && s.pmx_path.is_some()
        }));
        assert!(stages
            .iter()
            .any(|s| s.name == "empty_stage" && matches!(s.status, StageStatus::Invalid)));
    }

    #[test]
    fn discover_pmx_files_recurses_into_nested_directories() {
        let dir = tempdir().expect("tempdir");
        let pmx_root = dir.path().join("assets").join("pmx");
        let nested_dir = pmx_root.join("miku").join("tex");
        fs::create_dir_all(&nested_dir).expect("pmx dirs");
        let pmx_path = pmx_root.join("miku").join("Tda式初音ミクV4X_Ver1.00.pmx");
        fs::write(&pmx_path, b"pmx").expect("pmx file");
        fs::write(nested_dir.join("toon_defo.bmp"), b"tex").expect("texture file");

        let files = discover_pmx_files(&pmx_root).expect("discover pmx files");
        assert_eq!(files, vec![pmx_path]);
    }

    #[test]
    fn discover_vmd_files_keeps_motion_and_camera_dirs_separate() {
        let dir = tempdir().expect("tempdir");
        let motion_dir = dir.path().join("assets").join("vmd");
        let camera_dir = dir.path().join("assets").join("camera");
        fs::create_dir_all(&motion_dir).expect("motion dir");
        fs::create_dir_all(&camera_dir).expect("camera dir");
        let motion_vmd = motion_dir.join("dance.vmd");
        let camera_vmd = camera_dir.join("world_is_mine.vmd");
        fs::write(&motion_vmd, b"motion").expect("motion file");
        fs::write(&camera_vmd, b"camera").expect("camera file");

        let motion_files = discover_vmd_files(&motion_dir);
        let camera_files = discover_camera_vmds(&camera_dir);

        assert_eq!(motion_files, vec![motion_vmd]);
        assert_eq!(camera_files, vec![camera_vmd]);
    }

    #[test]
    fn stage_selector_supports_auto_none_and_name() {
        let stages = vec![
            StageChoice {
                name: "alpha".to_owned(),
                status: StageStatus::NeedsConvert,
                render_path: None,
                pmx_path: Some(PathBuf::from("alpha/stage.pmx")),
                transform: StageTransform::default(),
            },
            StageChoice {
                name: "beta".to_owned(),
                status: StageStatus::Ready,
                render_path: Some(PathBuf::from("beta/stage.glb")),
                pmx_path: None,
                transform: StageTransform::default(),
            },
        ];

        let auto = resolve_stage_choice_from_selector(&stages, "auto");
        assert_eq!(auto.as_ref().map(|s| s.name.as_str()), Some("beta"));

        let none = resolve_stage_choice_from_selector(&stages, "none");
        assert!(none.is_none());

        let named = resolve_stage_choice_from_selector(&stages, "beta");
        assert_eq!(named.as_ref().map(|s| s.name.as_str()), Some("beta"));
    }

    #[test]
    fn discover_default_camera_prefers_world_is_mine() {
        let dir = tempdir().expect("tempdir");
        let camera_dir = dir.path().join("assets").join("camera");
        fs::create_dir_all(&camera_dir).expect("camera dir");
        fs::write(camera_dir.join("a.vmd"), b"vmd").expect("a");
        fs::write(camera_dir.join("world_is_mine.vmd"), b"vmd").expect("world");
        let picked = asset_discovery::discover_default_camera_vmd(&camera_dir).expect("picked");
        assert_eq!(
            picked.file_name().and_then(|value| value.to_str()),
            Some("world_is_mine.vmd")
        );
    }

    #[test]
    fn distance_clamp_guard_pushes_camera_outside_min_radius() {
        let mut guard = DistanceClampGuard::default();
        let target = Vec3::ZERO;
        let mut camera = Camera {
            eye: Vec3::new(0.05, 0.0, 0.03),
            target,
            up: Vec3::Y,
        };
        let min_dist = guard.apply(&mut camera, target, 1.0, 1.0);
        let actual = (camera.eye - target).length();
        assert!(actual + 1e-4 >= min_dist);
        assert!(min_dist >= 0.35);
    }

    #[test]
    fn dynamic_clip_planes_remain_valid() {
        let (near, far) = dynamic_clip_planes(0.6, 1.4, 2.0, false);
        assert!(near > 0.0);
        assert!(far > near);
        assert!(near <= 0.10);
        assert!(far <= 500.0);
    }

    #[test]
    fn dynamic_clip_planes_expand_far_for_stage() {
        let (_, far_no_stage) = dynamic_clip_planes(0.6, 1.4, 2.0, false);
        let (_, far_with_stage) = dynamic_clip_planes(0.6, 1.4, 8.0, true);
        assert!(far_with_stage > far_no_stage);
    }
}
