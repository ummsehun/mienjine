use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use crate::{
    assets::vmd_motion::parse_vmd_motion,
    cli::{BenchArgs, Cli, Commands, PreprocessArgs, RunArgs, RunSceneArg},
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
        graphics_proto::cleanup_orphan_shm_files,
        interaction::max_scene_vertices,
        options::{
            resolve_effective_camera_mode, resolve_pmx_settings_for_run,
            resolve_sync_options_for_run, resolve_sync_profile_for_assets,
            resolve_sync_profile_options_for_run, resolve_visual_options_for_bench,
            resolve_visual_options_for_run,
        },
        preprocess::run_preprocess,
        render_loop::run_scene_interactive,
        start_ui::StageStatus,
        state::{resolve_runtime_backend, RuntimeCameraSettings},
    },
    scene::{resolve_cell_aspect, CellAspectMode},
};

mod common;
mod config;
mod panic_state;
mod preview_cmd;

pub(crate) use common::resolve_animation_index;
pub(crate) use config::{
    apply_runtime_render_tuning, load_runtime_config, persist_sync_profile_offset,
};
pub(crate) use panic_state::set_runtime_panic_state;

use common::{load_scene_for_bench, load_scene_for_run};
use panic_state::install_runtime_panic_hook_once;
use preview_cmd::preview;

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
    let pmx_settings = resolve_pmx_settings_for_run(&args, &runtime_cfg);
    let (mut scene, mut animation_index, rotates_without_animation) = load_scene_for_run(&args)?;

    if matches!(args.scene, RunSceneArg::Pmx) {
        if let Some(motion_path) = args.motion_vmd.as_deref() {
            match parse_vmd_motion(motion_path) {
                Ok(vmd) => {
                    let clip = vmd.to_clip_for_scene(&scene);
                    if clip.channels.is_empty() {
                        eprintln!(
                            "warning: VMD clip has no matched channels; bone/morph names may not match this PMX."
                        );
                    }
                    scene.animations.push(clip);
                    animation_index = if scene.animations.is_empty() {
                        None
                    } else {
                        Some(scene.animations.len() - 1)
                    };
                }
                Err(err) => {
                    eprintln!(
                        "warning: failed to parse PMX motion VMD {}: {err}",
                        motion_path.display()
                    );
                }
            }
        }
    }
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

    let clip_duration_secs = animation_index
        .and_then(|idx| scene.animations.get(idx))
        .map(|clip| clip.duration);
    let audio_sync = crate::runtime::audio_sync::prepare_audio_sync(
        args.music.as_deref(),
        clip_duration_secs,
        sync.sync_speed_mode,
    );
    if args.music.is_some() && audio_sync.is_none() {
        eprintln!("warning: audio playback unavailable. continuing in silent mode.");
    }

    if matches!(args.scene, RunSceneArg::Pmx)
        && args.motion_vmd.is_some()
        && animation_index.is_none()
    {
        eprintln!("warning: no animation clip was selected after PMX+VMD import.");
    }

    run_scene_interactive(
        scene,
        animation_index,
        rotates_without_animation,
        config,
        audio_sync,
        sync.sync_offset_ms,
        args.orbit_speed,
        args.orbit_radius,
        args.camera_height,
        args.look_at_y,
        visual.wasd_mode,
        visual.freefly_speed,
        camera_settings,
        pmx_settings,
        sync_profile_context,
    )
}

fn preprocess(args: PreprocessArgs) -> Result<()> {
    run_preprocess(&args)
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
        pipeline.prepare_frame(&scene, elapsed, animation_index, None, 0.0);
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

#[cfg(test)]
mod tests;
