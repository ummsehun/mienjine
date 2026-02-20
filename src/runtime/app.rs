use std::{
    fs,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::window_size;
use glam::Vec3;
use rodio::{Decoder, OutputStream, Sink, Source};

use crate::{
    animation::{compute_global_matrices, default_poses},
    cli::{BenchArgs, BenchSceneArg, Cli, Commands, InspectArgs, RunArgs, RunSceneArg, StartArgs},
    loader,
    pipeline::FramePipeline,
    renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, render_frame},
    runtime::{
        config::{GasciiConfig, load_gascii_config},
        start_ui::{StartWizardDefaults, run_start_wizard},
    },
    scene::{
        CellAspectMode, ContrastProfile, RenderConfig, SceneCpu, SyncSpeedMode,
        estimate_cell_aspect_from_window, resolve_cell_aspect,
    },
    terminal::TerminalSession,
};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Start(args) => start(args),
        Commands::Run(args) => run_interactive(args),
        Commands::Bench(args) => bench(args),
        Commands::Inspect(args) => inspect(args),
    }
}

const SYNC_OFFSET_STEP_MS: i32 = 10;
const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeContrastPreset {
    AdaptiveLow,
    AdaptiveNormal,
    AdaptiveHigh,
    Fixed,
}

impl RuntimeContrastPreset {
    fn from_profile(profile: ContrastProfile) -> Self {
        match profile {
            ContrastProfile::Adaptive => RuntimeContrastPreset::AdaptiveNormal,
            ContrastProfile::Fixed => RuntimeContrastPreset::Fixed,
        }
    }

    fn next(self) -> Self {
        match self {
            RuntimeContrastPreset::AdaptiveLow => RuntimeContrastPreset::AdaptiveNormal,
            RuntimeContrastPreset::AdaptiveNormal => RuntimeContrastPreset::AdaptiveHigh,
            RuntimeContrastPreset::AdaptiveHigh => RuntimeContrastPreset::Fixed,
            RuntimeContrastPreset::Fixed => RuntimeContrastPreset::AdaptiveLow,
        }
    }

    fn label(self) -> &'static str {
        match self {
            RuntimeContrastPreset::AdaptiveLow => "adaptive-low",
            RuntimeContrastPreset::AdaptiveNormal => "adaptive-normal",
            RuntimeContrastPreset::AdaptiveHigh => "adaptive-high",
            RuntimeContrastPreset::Fixed => "fixed",
        }
    }
}

struct AudioSyncRuntime {
    playback: MusicPlayback,
    speed_factor: f32,
}

#[derive(Debug, Default, Clone, Copy)]
struct RuntimeInputResult {
    quit: bool,
    status_changed: bool,
}

fn start(args: StartArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_start(&args, runtime_cfg);
    let sync_defaults = resolve_sync_options_for_start(&args, runtime_cfg);
    let model_files = discover_glb_files(&args.dir)?;
    if model_files.is_empty() {
        bail!(
            "no .glb/.gltf files found in {}",
            args.dir.as_path().display()
        );
    }
    let music_files = discover_music_files(&args.music_dir)?;
    let defaults = StartWizardDefaults {
        mode: args.mode.into(),
        fps_cap: args.fps_cap,
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        contrast_profile: visual.contrast_profile,
        sync_offset_ms: sync_defaults.sync_offset_ms,
        sync_speed_mode: sync_defaults.sync_speed_mode,
        font_preset_enabled: runtime_cfg.font_preset_enabled,
    };
    let Some(selection) = run_start_wizard(
        &args.dir,
        &args.music_dir,
        &model_files,
        &music_files,
        defaults,
        runtime_cfg.ui_language,
        args.anim.as_deref(),
    )?
    else {
        return Ok(());
    };
    if selection.apply_font_preset {
        apply_startup_font_config(runtime_cfg);
    }
    let scene = loader::load_gltf(&selection.glb_path)?;
    let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
    let clip_duration_secs = animation_index
        .and_then(|idx| scene.animations.get(idx))
        .map(|clip| clip.duration);
    let audio_sync = prepare_audio_sync(
        selection.music_path.as_deref(),
        clip_duration_secs,
        selection.sync_speed_mode,
    );
    if selection.music_path.is_some() && audio_sync.is_none() {
        eprintln!("warning: audio playback unavailable. continuing in silent mode.");
    }
    let mut config = render_config_from_start(
        &args,
        ResolvedVisualOptions {
            cell_aspect_mode: selection.cell_aspect_mode,
            cell_aspect_trim: selection.cell_aspect_trim,
            contrast_profile: selection.contrast_profile,
        },
    );
    config.mode = selection.mode;
    config.fps_cap = selection.fps_cap;
    config.cell_aspect = selection.cell_aspect;
    apply_runtime_render_tuning(&mut config, runtime_cfg);
    run_scene_interactive(
        scene,
        animation_index,
        false,
        config,
        audio_sync,
        selection.sync_offset_ms,
        args.orbit_speed,
        args.orbit_radius,
        args.camera_height,
        args.look_at_y,
    )
}

fn run_interactive(args: RunArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_run(&args, runtime_cfg);
    let sync = resolve_sync_options_for_run(&args, runtime_cfg);
    let (scene, animation_index, rotates_without_animation) = load_scene_for_run(&args)?;
    let mut config = render_config_from_run(&args, visual);
    apply_runtime_render_tuning(&mut config, runtime_cfg);
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
    )
}

fn load_runtime_config() -> GasciiConfig {
    load_gascii_config(Path::new("Gascii.config"))
}

#[derive(Debug, Clone, Copy)]
struct ResolvedVisualOptions {
    cell_aspect_mode: CellAspectMode,
    cell_aspect_trim: f32,
    contrast_profile: ContrastProfile,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedSyncOptions {
    sync_offset_ms: i32,
    sync_speed_mode: SyncSpeedMode,
}

fn resolve_visual_options_for_start(
    args: &StartArgs,
    runtime_cfg: GasciiConfig,
) -> ResolvedVisualOptions {
    ResolvedVisualOptions {
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
    }
}

fn resolve_visual_options_for_run(
    args: &RunArgs,
    runtime_cfg: GasciiConfig,
) -> ResolvedVisualOptions {
    ResolvedVisualOptions {
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
    }
}

fn resolve_visual_options_for_bench(
    args: &BenchArgs,
    runtime_cfg: GasciiConfig,
) -> ResolvedVisualOptions {
    ResolvedVisualOptions {
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
    }
}

fn resolve_sync_options_for_start(
    args: &StartArgs,
    runtime_cfg: GasciiConfig,
) -> ResolvedSyncOptions {
    ResolvedSyncOptions {
        sync_offset_ms: args
            .sync_offset_ms
            .unwrap_or(runtime_cfg.sync_offset_ms)
            .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
        sync_speed_mode: args
            .sync_speed_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_speed_mode),
    }
}

fn resolve_sync_options_for_run(args: &RunArgs, runtime_cfg: GasciiConfig) -> ResolvedSyncOptions {
    ResolvedSyncOptions {
        sync_offset_ms: args
            .sync_offset_ms
            .unwrap_or(runtime_cfg.sync_offset_ms)
            .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
        sync_speed_mode: args
            .sync_speed_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_speed_mode),
    }
}

fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: GasciiConfig) {
    config.triangle_stride = runtime_cfg.triangle_stride.max(1);
    config.min_triangle_area_px2 = runtime_cfg.min_triangle_area_px2.max(0.0);
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
    }
}

fn run_scene_interactive(
    scene: SceneCpu,
    animation_index: Option<usize>,
    rotates_without_animation: bool,
    config: RenderConfig,
    audio_sync: Option<AudioSyncRuntime>,
    initial_sync_offset_ms: i32,
    orbit_speed: f32,
    orbit_radius: f32,
    camera_height: f32,
    look_at_y: f32,
) -> Result<()> {
    let mut terminal = TerminalSession::enter()?;
    let (width, height) = validated_terminal_size(&terminal)?;
    let mut frame = FrameBuffers::new(width, height);
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let framing = compute_scene_framing(&scene, &config, orbit_radius, camera_height, look_at_y);
    let mut orbit_speed = orbit_speed.max(0.0);
    let mut orbit_enabled = orbit_speed > 0.0;
    let mut model_spin_enabled = rotates_without_animation;
    let mut zoom = 1.0_f32;
    let mut focus_offset = Vec3::ZERO;
    let mut camera_height_offset = 0.0_f32;
    let mut sync_offset_ms =
        initial_sync_offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
    let mut contrast_preset = RuntimeContrastPreset::from_profile(config.contrast_profile);
    let mut osd_until: Option<Instant> = Some(Instant::now() + Duration::from_secs(2));

    let start = Instant::now();
    let frame_budget = Duration::from_secs_f32(1.0 / (config.fps_cap as f32));
    if let Some(audio) = audio_sync.as_ref() {
        audio.playback.sink.play();
    }

    loop {
        let frame_start = Instant::now();
        let input = process_runtime_input(
            &mut frame,
            &mut orbit_enabled,
            &mut orbit_speed,
            &mut model_spin_enabled,
            &mut zoom,
            &mut focus_offset,
            &mut camera_height_offset,
            &mut sync_offset_ms,
            &mut contrast_preset,
        )?;
        if input.quit {
            break;
        }
        if input.status_changed {
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }

        let elapsed_wall = start.elapsed().as_secs_f32();
        let sync_speed = audio_sync.as_ref().map(|s| s.speed_factor).unwrap_or(1.0);
        let elapsed_audio = audio_sync
            .as_ref()
            .map(|s| s.playback.sink.get_pos().as_secs_f32());
        let animation_time =
            compute_animation_time(elapsed_wall, elapsed_audio, sync_speed, sync_offset_ms);
        pipeline.prepare_frame(&scene, animation_time, animation_index);
        let rotation = if animation_index.is_some() {
            0.0
        } else if model_spin_enabled {
            elapsed_wall * 0.9
        } else {
            0.0
        };
        let detected_cell_aspect = detect_terminal_cell_aspect();
        let effective_aspect = resolve_cell_aspect(
            &config,
            if config.cell_aspect_mode == CellAspectMode::Auto {
                detected_cell_aspect
            } else {
                None
            },
        );
        let mut frame_config = config.clone();
        frame_config.cell_aspect_mode = CellAspectMode::Manual;
        frame_config.cell_aspect = effective_aspect;
        apply_runtime_contrast_preset(&mut frame_config, contrast_preset);
        let camera = orbit_camera(
            if orbit_enabled { orbit_speed } else { 0.0 },
            (framing.radius * zoom).clamp(0.2, 1000.0),
            (framing.camera_height + camera_height_offset).clamp(-1000.0, 1000.0),
            framing.focus + focus_offset,
            elapsed_wall,
        );
        render_frame(
            &mut frame,
            &frame_config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            &glyph_ramp,
            &mut render_scratch,
            camera,
            rotation,
        );
        if osd_until.is_some_and(|until| Instant::now() <= until) {
            let status = format_runtime_status(
                sync_offset_ms,
                sync_speed,
                effective_aspect,
                contrast_preset,
            );
            overlay_osd(&mut frame, &status);
        }
        let output = pipeline.text_buffer_mut();
        frame.write_text(output);
        terminal.draw_frame(output)?;

        let elapsed_frame = frame_start.elapsed();
        if elapsed_frame < frame_budget {
            thread::sleep(frame_budget - elapsed_frame);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CameraFraming {
    focus: Vec3,
    radius: f32,
    camera_height: f32,
}

fn compute_scene_framing(
    scene: &SceneCpu,
    config: &RenderConfig,
    user_orbit_radius: f32,
    user_camera_height: f32,
    user_look_at_y: f32,
) -> CameraFraming {
    let Some(stats) = scene_stats_world(scene) else {
        return CameraFraming {
            focus: Vec3::new(
                0.0,
                if user_look_at_y != 0.0 {
                    user_look_at_y
                } else {
                    1.0
                },
                0.0,
            ),
            radius: user_orbit_radius.max(0.1),
            camera_height: if user_camera_height != 0.0 {
                user_camera_height
            } else {
                1.2
            },
        };
    };

    let extent = (stats.max - stats.min).abs();
    let auto_focus_y = (stats.min.y + stats.max.y) * 0.5;
    let focus = Vec3::new(
        stats.median.x,
        if user_look_at_y != 0.0 {
            user_look_at_y
        } else {
            auto_focus_y
        },
        stats.median.z,
    );

    let fov_rad = config.fov_deg.to_radians().clamp(0.35, 2.6);
    let object_radius = stats
        .p98_distance
        .max(stats.p90_distance * 1.12)
        .max(extent.y * 0.52)
        .max(extent.x * 0.46)
        .max(0.25);
    let mut auto_radius = object_radius / (fov_rad * 0.5).tan();
    auto_radius = (auto_radius * 1.08).max(1.2);
    let auto_height = focus.y + extent.y.max(0.3) * 0.02;

    CameraFraming {
        focus,
        radius: if user_orbit_radius > 0.0 {
            user_orbit_radius
        } else {
            auto_radius
        },
        camera_height: if user_camera_height != 0.0 {
            user_camera_height
        } else {
            auto_height
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct SceneStats {
    min: Vec3,
    max: Vec3,
    median: Vec3,
    p90_distance: f32,
    p98_distance: f32,
}

fn scene_stats_world(scene: &SceneCpu) -> Option<SceneStats> {
    if scene.mesh_instances.is_empty() {
        return None;
    }
    let poses = default_poses(&scene.nodes);
    let globals = compute_global_matrices(&scene.nodes, &poses);

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut points = Vec::new();
    for instance in &scene.mesh_instances {
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let node_global = globals
            .get(instance.node_index)
            .copied()
            .unwrap_or(glam::Mat4::IDENTITY);
        for position in &mesh.positions {
            let p = node_global.transform_point3(*position);
            min = min.min(p);
            max = max.max(p);
            points.push(p);
        }
    }
    if points.is_empty() {
        return None;
    }

    let mut xs = points.iter().map(|p| p.x).collect::<Vec<_>>();
    let mut ys = points.iter().map(|p| p.y).collect::<Vec<_>>();
    let mut zs = points.iter().map(|p| p.z).collect::<Vec<_>>();
    xs.sort_by(f32::total_cmp);
    ys.sort_by(f32::total_cmp);
    zs.sort_by(f32::total_cmp);

    let q01 = Vec3::new(
        quantile_sorted(&xs, 0.01),
        quantile_sorted(&ys, 0.01),
        quantile_sorted(&zs, 0.01),
    );
    let q99 = Vec3::new(
        quantile_sorted(&xs, 0.99),
        quantile_sorted(&ys, 0.99),
        quantile_sorted(&zs, 0.99),
    );
    let median = Vec3::new(
        quantile_sorted(&xs, 0.50),
        quantile_sorted(&ys, 0.50),
        quantile_sorted(&zs, 0.50),
    );

    let mut robust_min = q01;
    let mut robust_max = q99;
    if (robust_max - robust_min).abs().length_squared() < 1e-6 {
        robust_min = min;
        robust_max = max;
    }

    let mut distances = Vec::with_capacity(points.len());
    for p in &points {
        if p.x >= robust_min.x
            && p.x <= robust_max.x
            && p.y >= robust_min.y
            && p.y <= robust_max.y
            && p.z >= robust_min.z
            && p.z <= robust_max.z
        {
            distances.push((*p - median).length());
        }
    }
    if distances.is_empty() {
        distances.extend(points.iter().map(|p| (*p - median).length()));
    }
    distances.sort_by(f32::total_cmp);
    let p90_distance = quantile_sorted(&distances, 0.90).max(0.05);
    let p98_distance = quantile_sorted(&distances, 0.98).max(p90_distance);

    Some(SceneStats {
        min: robust_min,
        max: robust_max,
        median,
        p90_distance,
        p98_distance,
    })
}

fn quantile_sorted(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let q = q.clamp(0.0, 1.0);
    let pos = q * ((sorted.len() - 1) as f32);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let t = pos - (lo as f32);
    sorted[lo] * (1.0 - t) + sorted[hi] * t
}

fn render_config_from_run(args: &RunArgs, visual: ResolvedVisualOptions) -> RenderConfig {
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode: args.mode.into(),
        charset: args.charset.clone(),
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        fps_cap: args.fps_cap.max(1),
        ambient: args.ambient,
        diffuse_strength: args.diffuse_strength,
        specular_strength: args.specular_strength,
        specular_power: args.specular_power,
        rim_strength: args.rim_strength,
        rim_power: args.rim_power,
        fog_strength: args.fog_strength,
        contrast_profile: visual.contrast_profile,
        contrast_floor: 0.10,
        contrast_gamma: 0.90,
        fog_scale: 1.0,
        triangle_stride: 1,
        min_triangle_area_px2: 0.0,
    }
}

fn render_config_from_start(args: &StartArgs, visual: ResolvedVisualOptions) -> RenderConfig {
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode: args.mode.into(),
        charset: args.charset.clone(),
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        fps_cap: args.fps_cap.max(1),
        ambient: args.ambient,
        diffuse_strength: args.diffuse_strength,
        specular_strength: args.specular_strength,
        specular_power: args.specular_power,
        rim_strength: args.rim_strength,
        rim_power: args.rim_power,
        fog_strength: args.fog_strength,
        contrast_profile: visual.contrast_profile,
        contrast_floor: 0.10,
        contrast_gamma: 0.90,
        fog_scale: 1.0,
        triangle_stride: 1,
        min_triangle_area_px2: 0.0,
    }
}

fn discover_glb_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;
    let mut files = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let lower = ext.to_ascii_lowercase();
                    lower == "glb" || lower == "gltf"
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn discover_music_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;
    let mut files = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let lower = ext.to_ascii_lowercase();
                    lower == "mp3" || lower == "wav"
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn validated_terminal_size(terminal: &TerminalSession) -> Result<(u16, u16)> {
    let (w, h) = terminal.size()?;
    if w > 0 && h > 0 {
        return Ok((w, h));
    }
    let env_w = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0);
    let env_h = std::env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0);
    match (env_w, env_h) {
        (Some(width), Some(height)) => Ok((width, height)),
        _ => bail!(
            "terminal size unavailable (got {w}x{h}). set COLUMNS/LINES or use a real TTY terminal"
        ),
    }
}

fn apply_startup_font_config(runtime_cfg: GasciiConfig) {
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

fn run_ghostty_font_shortcut(key: &str) {
    if !running_in_ghostty() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Ghostty\" to activate\ntell application \"System Events\" to keystroke \"{}\" using command down",
            key
        );
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = key;
    }
}

fn running_in_ghostty() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|v| v.eq_ignore_ascii_case("ghostty"))
        .unwrap_or(false)
}

struct MusicPlayback {
    _stream: OutputStream,
    sink: Sink,
    duration_secs: Option<f32>,
}

impl Drop for MusicPlayback {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

fn start_music_playback(path: Option<&Path>) -> Option<MusicPlayback> {
    let path = path?;
    let stream = OutputStream::try_default().ok()?;
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let duration_secs = decoder.total_duration().map(|d| d.as_secs_f32());
    let sink = Sink::try_new(&stream.1).ok()?;
    sink.pause();
    sink.append(decoder.repeat_infinite());
    Some(MusicPlayback {
        _stream: stream.0,
        sink,
        duration_secs,
    })
}

fn prepare_audio_sync(
    music_path: Option<&Path>,
    clip_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> Option<AudioSyncRuntime> {
    let playback = start_music_playback(music_path)?;
    let speed_factor =
        compute_animation_speed_factor(clip_duration_secs, playback.duration_secs, mode);
    if matches!(mode, SyncSpeedMode::AutoDurationFit) && (speed_factor - 1.0).abs() > 1e-4 {
        eprintln!(
            "info: audio sync speed factor applied {:.4} (clip={:?}s, audio={:?}s)",
            speed_factor, clip_duration_secs, playback.duration_secs
        );
    }
    Some(AudioSyncRuntime {
        playback,
        speed_factor,
    })
}

fn compute_animation_speed_factor(
    clip_duration_secs: Option<f32>,
    audio_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> f32 {
    if !matches!(mode, SyncSpeedMode::AutoDurationFit) {
        return 1.0;
    }
    let Some(clip) = clip_duration_secs else {
        return 1.0;
    };
    let Some(audio) = audio_duration_secs else {
        return 1.0;
    };
    if clip <= f32::EPSILON || audio <= f32::EPSILON {
        return 1.0;
    }
    let factor = clip / audio;
    if (0.85..=1.15).contains(&factor) {
        factor
    } else {
        eprintln!(
            "warning: sync speed factor {:.4} out of range [0.85, 1.15], fallback to 1.0",
            factor
        );
        1.0
    }
}

fn compute_animation_time(
    elapsed_wall: f32,
    elapsed_audio: Option<f32>,
    speed_factor: f32,
    sync_offset_ms: i32,
) -> f32 {
    let offset = (sync_offset_ms as f32) / 1000.0;
    elapsed_audio
        .map(|seconds| seconds * speed_factor + offset)
        .unwrap_or(elapsed_wall + offset)
}

fn detect_terminal_cell_aspect() -> Option<f32> {
    let ws = window_size().ok()?;
    estimate_cell_aspect_from_window(ws.columns, ws.rows, ws.width, ws.height)
}

fn apply_runtime_contrast_preset(config: &mut RenderConfig, preset: RuntimeContrastPreset) {
    match preset {
        RuntimeContrastPreset::AdaptiveLow => {
            config.contrast_profile = ContrastProfile::Adaptive;
            config.contrast_floor = 0.08;
            config.contrast_gamma = 1.00;
            config.fog_scale = 1.00;
        }
        RuntimeContrastPreset::AdaptiveNormal => {
            config.contrast_profile = ContrastProfile::Adaptive;
            config.contrast_floor = 0.10;
            config.contrast_gamma = 0.90;
            config.fog_scale = 1.00;
        }
        RuntimeContrastPreset::AdaptiveHigh => {
            config.contrast_profile = ContrastProfile::Adaptive;
            config.contrast_floor = 0.14;
            config.contrast_gamma = 0.78;
            config.fog_scale = 0.80;
        }
        RuntimeContrastPreset::Fixed => {}
    }
}

fn format_runtime_status(
    sync_offset_ms: i32,
    sync_speed: f32,
    effective_aspect: f32,
    contrast: RuntimeContrastPreset,
) -> String {
    format!(
        "offset={sync_offset_ms}ms  speed={sync_speed:.4}x  aspect={effective_aspect:.3}  contrast={}",
        contrast.label()
    )
}

fn overlay_osd(frame: &mut FrameBuffers, text: &str) {
    if frame.width == 0 || frame.height == 0 {
        return;
    }
    let width = usize::from(frame.width);
    let y = usize::from(frame.height.saturating_sub(1));
    let row_start = y * width;
    let row_end = row_start + width;
    for glyph in &mut frame.glyphs[row_start..row_end] {
        *glyph = ' ';
    }
    for (i, ch) in text.chars().take(width).enumerate() {
        frame.glyphs[row_start + i] = ch;
    }
}

fn process_runtime_input(
    frame: &mut FrameBuffers,
    orbit_enabled: &mut bool,
    orbit_speed: &mut f32,
    model_spin_enabled: &mut bool,
    zoom: &mut f32,
    focus_offset: &mut Vec3,
    camera_height_offset: &mut f32,
    sync_offset_ms: &mut i32,
    contrast_preset: &mut RuntimeContrastPreset,
) -> Result<RuntimeInputResult> {
    let mut result = RuntimeInputResult::default();
    while event::poll(Duration::from_millis(0))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    result.quit = true;
                    return Ok(result);
                }
                KeyCode::Char('o') | KeyCode::Char('O') => *orbit_enabled = !*orbit_enabled,
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    *model_spin_enabled = !*model_spin_enabled
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    *orbit_speed = (*orbit_speed + 0.05).clamp(0.0, 3.0);
                    if *orbit_speed > 0.0 {
                        *orbit_enabled = true;
                    }
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    *orbit_speed = (*orbit_speed - 0.05).clamp(0.0, 3.0);
                }
                KeyCode::Char('[') => *zoom = (*zoom + 0.08).clamp(0.2, 8.0),
                KeyCode::Char(']') => *zoom = (*zoom - 0.08).clamp(0.2, 8.0),
                KeyCode::Left | KeyCode::Char('j') | KeyCode::Char('J') => focus_offset.x -= 0.08,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => focus_offset.x += 0.08,
                KeyCode::Up | KeyCode::Char('i') | KeyCode::Char('I') => {
                    focus_offset.y += 0.08;
                    *camera_height_offset += 0.08;
                }
                KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('K') => {
                    focus_offset.y -= 0.08;
                    *camera_height_offset -= 0.08;
                }
                KeyCode::Char('u') | KeyCode::Char('U') => focus_offset.z += 0.08,
                KeyCode::Char('m') | KeyCode::Char('M') => focus_offset.z -= 0.08,
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    *zoom = 1.0;
                    *focus_offset = Vec3::ZERO;
                    *camera_height_offset = 0.0;
                }
                KeyCode::Char(',') => {
                    *sync_offset_ms = (*sync_offset_ms - SYNC_OFFSET_STEP_MS)
                        .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
                    result.status_changed = true;
                }
                KeyCode::Char('.') => {
                    *sync_offset_ms = (*sync_offset_ms + SYNC_OFFSET_STEP_MS)
                        .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
                    result.status_changed = true;
                }
                KeyCode::Char('/') => {
                    *sync_offset_ms = 0;
                    result.status_changed = true;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    *contrast_preset = contrast_preset.next();
                    result.status_changed = true;
                }
                _ => {}
            },
            Event::Resize(width, height) => {
                frame.resize(width.max(1), height.max(1));
                result.status_changed = true;
            }
            _ => {}
        }
    }
    Ok(result)
}

fn bench(args: BenchArgs) -> Result<()> {
    let (scene, animation_index, rotates) = load_scene_for_bench(&args)?;
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_bench(&args, runtime_cfg);
    let mut config = RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode: args.mode.into(),
        charset: args.charset,
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        fps_cap: u32::MAX,
        ambient: args.ambient,
        diffuse_strength: args.diffuse_strength,
        specular_strength: args.specular_strength,
        specular_power: args.specular_power,
        rim_strength: args.rim_strength,
        rim_power: args.rim_power,
        fog_strength: args.fog_strength,
        contrast_profile: visual.contrast_profile,
        contrast_floor: 0.10,
        contrast_gamma: 0.90,
        fog_scale: 1.0,
        triangle_stride: 1,
        min_triangle_area_px2: 0.0,
    };
    apply_runtime_render_tuning(&mut config, runtime_cfg);
    config.cell_aspect = resolve_cell_aspect(&config, None);
    config.cell_aspect_mode = CellAspectMode::Manual;
    let mut frame = FrameBuffers::new(args.width.max(1), args.height.max(1));
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let camera = Camera::default();

    let benchmark_duration = Duration::from_secs_f32(args.seconds.max(0.1));
    let started = Instant::now();
    let mut frames: u64 = 0;
    let mut triangles: u64 = 0;
    let mut pixels: u64 = 0;

    while started.elapsed() < benchmark_duration {
        let elapsed = started.elapsed().as_secs_f32();
        pipeline.prepare_frame(&scene, elapsed, animation_index);
        let stats = render_frame(
            &mut frame,
            &config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
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

fn inspect(args: InspectArgs) -> Result<()> {
    let scene = loader::load_gltf(&args.glb)?;
    println!("file: {}", args.glb.display());
    println!("meshes: {}", scene.meshes.len());
    println!("mesh_instances: {}", scene.mesh_instances.len());
    println!("nodes: {}", scene.nodes.len());
    println!("skins: {}", scene.skins.len());
    println!("animations: {}", scene.animations.len());
    println!("total_vertices: {}", scene.total_vertices());
    println!("total_triangles: {}", scene.total_triangles());
    println!("total_joints: {}", scene.total_joints());
    if let Some(stats) = scene_stats_world(&scene) {
        let extent = (stats.max - stats.min).abs();
        let framing = compute_scene_framing(&scene, &RenderConfig::default(), 0.0, 0.0, 0.0);
        println!(
            "robust_bounds_min: [{:.4}, {:.4}, {:.4}]",
            stats.min.x, stats.min.y, stats.min.z
        );
        println!(
            "robust_bounds_max: [{:.4}, {:.4}, {:.4}]",
            stats.max.x, stats.max.y, stats.max.z
        );
        println!(
            "robust_extent: [{:.4}, {:.4}, {:.4}]",
            extent.x, extent.y, extent.z
        );
        println!(
            "median_center: [{:.4}, {:.4}, {:.4}]",
            stats.median.x, stats.median.y, stats.median.z
        );
        println!("distance_p90: {:.4}", stats.p90_distance);
        println!("distance_p98: {:.4}", stats.p98_distance);
        println!(
            "auto_frame: focus=[{:.4}, {:.4}, {:.4}] radius={:.4} camera_height={:.4}",
            framing.focus.x,
            framing.focus.y,
            framing.focus.z,
            framing.radius,
            framing.camera_height
        );
    }
    for (index, animation) in scene.animations.iter().enumerate() {
        println!(
            "animation[{index}]: name={} duration={:.3}s channels={}",
            animation.name.as_deref().unwrap_or("<unnamed>"),
            animation.duration,
            animation.channels.len()
        );
    }
    Ok(())
}

fn resolve_animation_index(scene: &SceneCpu, selector: Option<&str>) -> Result<Option<usize>> {
    if let Some(selector) = selector {
        let index = scene
            .animation_index_by_selector(Some(selector))
            .with_context(|| format!("animation selector not found: {selector}"))?;
        return Ok(Some(index));
    }
    Ok((!scene.animations.is_empty()).then_some(0))
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

fn orbit_camera(
    orbit_speed: f32,
    orbit_radius: f32,
    camera_height: f32,
    focus: Vec3,
    time: f32,
) -> Camera {
    let (eye_x, eye_z) = if orbit_speed.abs() <= f32::EPSILON {
        (focus.x, focus.z + orbit_radius)
    } else {
        let angle = time * orbit_speed + std::f32::consts::FRAC_PI_2;
        (
            focus.x + angle.cos() * orbit_radius,
            focus.z + angle.sin() * orbit_radius,
        )
    };
    let eye = Vec3::new(eye_x, camera_height, eye_z);
    let target = focus;
    Camera {
        eye,
        target,
        up: Vec3::Y,
    }
}

fn max_scene_vertices(scene: &SceneCpu) -> usize {
    scene
        .meshes
        .iter()
        .map(|mesh| mesh.positions.len())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn auto_speed_factor_clamps_outliers_to_one() {
        let factor = compute_animation_speed_factor(
            Some(300.0),
            Some(120.0),
            SyncSpeedMode::AutoDurationFit,
        );
        assert!((factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn animation_time_applies_sync_offset_with_audio_clock() {
        let time = compute_animation_time(5.0, Some(3.0), 1.05, 120);
        assert!((time - 3.27).abs() < 1e-6);
    }

    #[test]
    fn auto_framing_focus_y_uses_center() {
        let scene = crate::scene::cube_scene();
        let framing = compute_scene_framing(&scene, &RenderConfig::default(), 0.0, 0.0, 0.0);
        assert!(framing.focus.y.abs() < 0.05);
    }
}
