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

pub(crate) fn load_runtime_config() -> GasciiConfig {
    load_gascii_config(Path::new("Gascii.config"))
}

pub(crate) fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: &GasciiConfig) {
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

pub(crate) fn resolve_animation_index(
    scene: &SceneCpu,
    selector: Option<&str>,
) -> Result<Option<usize>> {
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
#[path = "app_impl/tests.rs"]
mod tests;
