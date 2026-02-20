use std::{
    fs,
    fs::File,
    io::BufReader,
    panic,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, Once, OnceLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::window_size;
use glam::Vec3;
use rodio::{Decoder, OutputStream, Sink, Source};

use crate::{
    animation::{ChannelTarget, compute_global_matrices, default_poses},
    cli::{BenchArgs, BenchSceneArg, Cli, Commands, InspectArgs, RunArgs, RunSceneArg, StartArgs},
    loader,
    pipeline::FramePipeline,
    render::backend::render_frame_with_backend,
    renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, RenderStats},
    runtime::{
        config::{GasciiConfig, load_gascii_config},
        start_ui::{StartWizardDefaults, run_start_wizard},
    },
    scene::{
        AudioReactiveMode, BrailleProfile, CameraFocusMode, CellAspectMode, CenterLockMode,
        CinematicCameraMode, ColorMode, ContrastProfile, DetailProfile, PerfProfile, RenderBackend,
        RenderConfig, RenderMode, SceneCpu, SyncSpeedMode, TextureSamplingMode, ThemeStyle,
        estimate_cell_aspect_from_window, resolve_cell_aspect,
    },
    terminal::{PresentMode, TerminalSession, supports_truecolor},
};

pub fn run(cli: Cli) -> Result<()> {
    install_runtime_panic_hook_once();
    match cli.command {
        Commands::Start(args) => start(args),
        Commands::Run(args) => run_interactive(args),
        Commands::Bench(args) => bench(args),
        Commands::Inspect(args) => inspect(args),
    }
}

const SYNC_OFFSET_STEP_MS: i32 = 10;
const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;
const MAX_RENDER_COLS: u16 = 600;
const MAX_RENDER_ROWS: u16 = 180;
const VISIBILITY_LOW_THRESHOLD: f32 = 0.002;
const VISIBILITY_LOW_FRAMES_TO_RECOVER: u32 = 12;
const MIN_VISIBLE_HEIGHT_RATIO: f32 = 0.10;
const MIN_VISIBLE_HEIGHT_TRIGGER_FRAMES: u32 = 10;
const MIN_VISIBLE_HEIGHT_RECOVER_RATIO: f32 = 0.16;
const MIN_VISIBLE_HEIGHT_RECOVER_FRAMES: u32 = 30;

static PANIC_HOOK_ONCE: Once = Once::new();
static LAST_RUNTIME_STATE: OnceLock<Mutex<String>> = OnceLock::new();

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
    envelope: Option<AudioEnvelope>,
}

#[derive(Debug, Clone)]
struct AudioEnvelope {
    fps: u32,
    values: Vec<f32>,
    duration_secs: f32,
}

impl AudioEnvelope {
    fn sample(&self, time_secs: f32) -> f32 {
        if self.values.is_empty() || self.duration_secs <= f32::EPSILON || self.fps == 0 {
            return 0.0;
        }
        let wrapped = time_secs.rem_euclid(self.duration_secs.max(f32::EPSILON));
        let idx = ((wrapped * self.fps as f32).floor() as usize) % self.values.len();
        self.values[idx].clamp(0.0, 1.0)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ReactiveState {
    energy: f32,
    smoothed_energy: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraShot {
    FullBody,
    UpperBody,
    FaceCloseup,
    Hands,
}

#[derive(Debug, Clone, Copy)]
struct CameraDirectorState {
    shot: CameraShot,
    next_cut_at: f32,
    transition_started_at: f32,
    previous_radius_mul: f32,
    previous_height_offset: f32,
    previous_focus_y_offset: f32,
    radius_mul: f32,
    height_offset: f32,
    focus_y_offset: f32,
    face_time_accum: f32,
    total_time_accum: f32,
    jitter_phase: f32,
}

impl Default for CameraDirectorState {
    fn default() -> Self {
        Self {
            shot: CameraShot::FullBody,
            next_cut_at: 6.0,
            transition_started_at: 0.0,
            previous_radius_mul: 1.0,
            previous_height_offset: 0.0,
            previous_focus_y_offset: 0.0,
            radius_mul: 1.0,
            height_offset: 0.0,
            focus_y_offset: 0.0,
            face_time_accum: 0.0,
            total_time_accum: 0.0,
            jitter_phase: 0.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct RuntimeInputResult {
    quit: bool,
    status_changed: bool,
    resized: bool,
    stage_changed: bool,
    center_lock_blocked_pan: bool,
    last_key: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct OrbitState {
    angle: f32,
    speed: f32,
    enabled: bool,
}

impl OrbitState {
    fn new(initial_speed: f32) -> Self {
        Self {
            angle: std::f32::consts::FRAC_PI_2,
            speed: initial_speed.max(0.0),
            enabled: initial_speed > 0.0,
        }
    }

    fn advance(&mut self, dt: f32) {
        if self.enabled && self.speed > 0.0 {
            self.angle += self.speed * dt.max(0.0);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RuntimeAdaptiveQuality {
    target_frame_ms: f32,
    ema_frame_ms: f32,
    lod_level: usize,
    overload_streak: u32,
    underload_streak: u32,
}

impl RuntimeAdaptiveQuality {
    fn new(profile: PerfProfile) -> Self {
        Self {
            target_frame_ms: target_frame_ms(profile),
            ema_frame_ms: target_frame_ms(profile),
            lod_level: 0,
            overload_streak: 0,
            underload_streak: 0,
        }
    }

    fn observe(&mut self, frame_ms: f32) -> bool {
        self.ema_frame_ms += (frame_ms - self.ema_frame_ms) * 0.12;
        let high = self.target_frame_ms * 1.18;
        let low = self.target_frame_ms * 0.82;
        let mut changed = false;

        if self.ema_frame_ms > high {
            self.overload_streak = self.overload_streak.saturating_add(1);
            self.underload_streak = 0;
            if self.overload_streak >= 20 && self.lod_level < 2 {
                self.lod_level += 1;
                self.overload_streak = 0;
                changed = true;
            }
        } else if self.ema_frame_ms < low {
            self.underload_streak = self.underload_streak.saturating_add(1);
            self.overload_streak = 0;
            if self.underload_streak >= 60 && self.lod_level > 0 {
                self.lod_level -= 1;
                self.underload_streak = 0;
                changed = true;
            }
        } else {
            self.overload_streak = 0;
            self.underload_streak = 0;
        }
        changed
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct VisibilityWatchdog {
    low_visible_streak: u32,
}

impl VisibilityWatchdog {
    fn observe(&mut self, visible_ratio: f32) -> bool {
        if visible_ratio < VISIBILITY_LOW_THRESHOLD {
            self.low_visible_streak = self.low_visible_streak.saturating_add(1);
        } else {
            self.low_visible_streak = 0;
        }
        self.low_visible_streak >= VISIBILITY_LOW_FRAMES_TO_RECOVER
    }

    fn reset(&mut self) {
        self.low_visible_streak = 0;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct CenterLockState {
    err_x_ema: f32,
    err_y_ema: f32,
    world_offset: Vec3,
}

impl CenterLockState {
    fn update(
        &mut self,
        stats: &RenderStats,
        mode: CenterLockMode,
        frame_width: u16,
        frame_height: u16,
        radius: f32,
        extent_y: f32,
    ) -> Vec3 {
        let anchor = match mode {
            CenterLockMode::Root => stats.root_screen_px.or(stats.visible_centroid_px),
            CenterLockMode::Mixed => match (stats.root_screen_px, stats.visible_centroid_px) {
                (Some(root), Some(centroid)) => Some((
                    root.0 * 0.7 + centroid.0 * 0.3,
                    root.1 * 0.7 + centroid.1 * 0.3,
                )),
                (Some(root), None) => Some(root),
                (None, Some(centroid)) => Some(centroid),
                (None, None) => None,
            },
        };
        let Some((cx, cy)) = anchor else {
            self.err_x_ema *= 0.85;
            self.err_y_ema *= 0.85;
            self.world_offset *= 0.92;
            return self.world_offset;
        };

        let fw = f32::from(frame_width.max(1));
        let fh = f32::from(frame_height.max(1));
        let nx = (cx / fw - 0.5) * 2.0;
        let ny = (cy / fh - 0.5) * 2.0;
        let dead_x = if nx.abs() < 0.015 { 0.0 } else { nx };
        let dead_y = if ny.abs() < 0.020 { 0.0 } else { ny };

        self.err_x_ema += (dead_x - self.err_x_ema) * 0.18;
        self.err_y_ema += (dead_y - self.err_y_ema) * 0.18;

        let radius = radius.max(0.2);
        let extent = extent_y.max(0.5);
        let target = Vec3::new(
            (-self.err_x_ema * radius * 0.32).clamp(-extent * 0.35, extent * 0.35),
            (-self.err_y_ema * extent * 0.28).clamp(-extent * 0.35, extent * 0.35),
            (self.err_x_ema * radius * 0.08).clamp(-extent * 0.35, extent * 0.35),
        );
        self.world_offset += (target - self.world_offset) * 0.12;
        self.world_offset
    }

    fn reset(&mut self) {
        self.err_x_ema = 0.0;
        self.err_y_ema = 0.0;
        self.world_offset = Vec3::ZERO;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct AutoRadiusGuard {
    low_height_streak: u32,
    recover_streak: u32,
    shrink_ratio: f32,
}

impl AutoRadiusGuard {
    fn update(&mut self, height_ratio: f32, enabled: bool) -> f32 {
        if !enabled {
            self.low_height_streak = 0;
            self.recover_streak = 0;
            self.shrink_ratio = 0.0;
            return 0.0;
        }

        if height_ratio < MIN_VISIBLE_HEIGHT_RATIO {
            self.low_height_streak = self.low_height_streak.saturating_add(1);
            self.recover_streak = 0;
            if self.low_height_streak >= MIN_VISIBLE_HEIGHT_TRIGGER_FRAMES {
                self.shrink_ratio = (self.shrink_ratio + 0.02).clamp(0.0, 0.12);
                self.low_height_streak = 0;
            }
        } else if height_ratio > MIN_VISIBLE_HEIGHT_RECOVER_RATIO {
            self.recover_streak = self.recover_streak.saturating_add(1);
            self.low_height_streak = 0;
            if self.recover_streak >= MIN_VISIBLE_HEIGHT_RECOVER_FRAMES {
                self.shrink_ratio = (self.shrink_ratio - 0.02).clamp(0.0, 0.12);
                self.recover_streak = 0;
            }
        } else {
            self.low_height_streak = 0;
            self.recover_streak = 0;
        }
        self.shrink_ratio
    }
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
    let start_mode: RenderMode = args.mode.into();
    let default_color_mode = visual
        .color_mode
        .unwrap_or_else(|| default_color_mode_for_mode(start_mode));
    let defaults = StartWizardDefaults {
        mode: start_mode,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        center_lock: visual.center_lock,
        center_lock_mode: visual.center_lock_mode,
        camera_focus: visual.camera_focus,
        material_color: visual.material_color,
        texture_sampling: visual.texture_sampling,
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
            perf_profile: selection.perf_profile,
            detail_profile: selection.detail_profile,
            backend: selection.backend,
            exposure_bias: visual.exposure_bias,
            center_lock: selection.center_lock,
            center_lock_mode: selection.center_lock_mode,
            camera_focus: selection.camera_focus,
            material_color: selection.material_color,
            texture_sampling: selection.texture_sampling,
            braille_aspect_compensation: selection.braille_aspect_compensation,
            stage_level: selection.stage_level,
            stage_reactive: selection.stage_reactive,
            color_mode: Some(selection.color_mode),
            braille_profile: selection.braille_profile,
            theme_style: selection.theme_style,
            audio_reactive: selection.audio_reactive,
            cinematic_camera: selection.cinematic_camera,
            reactive_gain: selection.reactive_gain,
        },
    );
    config.mode = selection.mode;
    config.perf_profile = selection.perf_profile;
    config.detail_profile = selection.detail_profile;
    config.backend = selection.backend;
    config.color_mode = selection.color_mode;
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
    config.stage_level = selection.stage_level;
    config.stage_reactive = selection.stage_reactive;
    config.material_color = selection.material_color;
    config.texture_sampling = selection.texture_sampling;
    config.braille_aspect_compensation = selection.braille_aspect_compensation;
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
    perf_profile: PerfProfile,
    detail_profile: DetailProfile,
    backend: RenderBackend,
    exposure_bias: f32,
    center_lock: bool,
    center_lock_mode: CenterLockMode,
    camera_focus: CameraFocusMode,
    material_color: bool,
    texture_sampling: TextureSamplingMode,
    braille_aspect_compensation: f32,
    stage_level: u8,
    stage_reactive: bool,
    color_mode: Option<ColorMode>,
    braille_profile: BrailleProfile,
    theme_style: ThemeStyle,
    audio_reactive: AudioReactiveMode,
    cinematic_camera: CinematicCameraMode,
    reactive_gain: f32,
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
        braille_aspect_compensation: runtime_cfg.braille_aspect_compensation,
        stage_level: args.stage_level.unwrap_or(runtime_cfg.stage_level).min(4),
        stage_reactive: runtime_cfg.stage_reactive,
        color_mode: args.color_mode.map(Into::into).or(runtime_cfg.color_mode),
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
        braille_aspect_compensation: runtime_cfg.braille_aspect_compensation,
        stage_level: args.stage_level.unwrap_or(runtime_cfg.stage_level).min(4),
        stage_reactive: runtime_cfg.stage_reactive,
        color_mode: args.color_mode.map(Into::into).or(runtime_cfg.color_mode),
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
        braille_aspect_compensation: runtime_cfg.braille_aspect_compensation,
        stage_level: args.stage_level.unwrap_or(runtime_cfg.stage_level).min(4),
        stage_reactive: runtime_cfg.stage_reactive,
        color_mode: args.color_mode.map(Into::into).or(runtime_cfg.color_mode),
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

fn default_color_mode_for_mode(mode: RenderMode) -> ColorMode {
    match mode {
        RenderMode::Braille => ColorMode::Ansi,
        RenderMode::Ascii => ColorMode::Mono,
    }
}

fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: GasciiConfig) {
    config.triangle_stride = runtime_cfg.triangle_stride.max(1);
    config.min_triangle_area_px2 = runtime_cfg.min_triangle_area_px2.max(0.0);
    config.braille_aspect_compensation = runtime_cfg.braille_aspect_compensation;
}

fn target_frame_ms(profile: PerfProfile) -> f32 {
    match profile {
        PerfProfile::Balanced => 33.3,
        PerfProfile::Cinematic => 50.0,
        PerfProfile::Smooth => 22.2,
    }
}

fn apply_adaptive_quality_tuning(
    config: &mut RenderConfig,
    base_triangle_stride: usize,
    base_min_triangle_area_px2: f32,
    lod_level: usize,
) {
    let mut effective_lod = lod_level;
    if matches!(config.detail_profile, DetailProfile::Perf) {
        effective_lod = effective_lod.max(1);
    }
    config.triangle_stride = base_triangle_stride.max(match effective_lod {
        0 => 1,
        1 => 2,
        _ => 3,
    });
    config.min_triangle_area_px2 = base_min_triangle_area_px2.max(match effective_lod {
        0 => 0.0,
        1 => 0.6,
        _ => 1.2,
    });

    if effective_lod >= 1 {
        config.texture_sampling = TextureSamplingMode::Nearest;
    }
    if effective_lod >= 2 && matches!(config.detail_profile, DetailProfile::Perf) {
        config.material_color = false;
    }
}

fn jitter_scale_for_lod(lod_level: usize) -> f32 {
    match lod_level {
        0 => 1.0,
        1 => 0.65,
        _ => 0.35,
    }
}

fn cap_render_size(width: u16, height: u16) -> (u16, u16, bool) {
    if width == 0 || height == 0 {
        return (1, 1, false);
    }
    if width <= MAX_RENDER_COLS && height <= MAX_RENDER_ROWS {
        return (width, height, false);
    }
    let scale_w = (MAX_RENDER_COLS as f32) / (width as f32);
    let scale_h = (MAX_RENDER_ROWS as f32) / (height as f32);
    let scale = scale_w.min(scale_h).clamp(0.01, 1.0);
    let capped_w = ((width as f32) * scale).floor() as u16;
    let capped_h = ((height as f32) * scale).floor() as u16;
    (capped_w.max(1), capped_h.max(1), true)
}

fn resolve_runtime_backend(requested: RenderBackend) -> RenderBackend {
    match requested {
        RenderBackend::Cpu => RenderBackend::Cpu,
        RenderBackend::Gpu => {
            #[cfg(all(feature = "gpu", target_os = "macos"))]
            {
                eprintln!("info: gpu backend enabled (experimental raster stage)");
                RenderBackend::Gpu
            }
            #[cfg(not(all(feature = "gpu", target_os = "macos")))]
            {
                eprintln!(
                    "warning: gpu backend unsupported in current build/platform. fallback to cpu."
                );
                RenderBackend::Cpu
            }
        }
    }
}

fn is_retryable_io_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(|io_err| {
                matches!(
                    io_err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                )
            })
            .unwrap_or(false)
    })
}

fn install_runtime_panic_hook_once() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            if let Some(lock) = LAST_RUNTIME_STATE.get() {
                if let Ok(state) = lock.lock() {
                    eprintln!("panic_state: {}", state.as_str());
                }
            }
            default_hook(panic_info);
        }));
    });
}

fn set_runtime_panic_state(line: String) {
    let lock = LAST_RUNTIME_STATE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut guard) = lock.lock() {
        *guard = line;
    }
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
    mut config: RenderConfig,
    audio_sync: Option<AudioSyncRuntime>,
    initial_sync_offset_ms: i32,
    orbit_speed: f32,
    orbit_radius: f32,
    camera_height: f32,
    look_at_y: f32,
) -> Result<()> {
    config.backend = resolve_runtime_backend(config.backend);
    let truecolor_supported = supports_truecolor();
    if matches!(config.color_mode, ColorMode::Ansi) && !truecolor_supported {
        eprintln!("warning: truecolor is unavailable in this terminal. fallback to mono mode.");
        config.color_mode = ColorMode::Mono;
    }
    let mut terminal = TerminalSession::enter()?;
    terminal.set_present_mode(PresentMode::Diff);
    let (term_width, term_height) = validated_terminal_size(&terminal)?;
    let (width, height, scaled) = cap_render_size(term_width, term_height);
    let mut frame = FrameBuffers::new(width, height);
    if scaled {
        eprintln!(
            "info: terminal size {}x{} capped to internal render {}x{}",
            term_width, term_height, width, height
        );
    }
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let framing = compute_scene_framing(&scene, &config, orbit_radius, camera_height, look_at_y);
    let scene_extent_y = scene
        .meshes
        .iter()
        .flat_map(|mesh| mesh.positions.iter().map(|p| p.y))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        });
    let extent_y = if scene_extent_y.0.is_finite() && scene_extent_y.1.is_finite() {
        (scene_extent_y.1 - scene_extent_y.0).abs().max(0.5)
    } else {
        1.0
    };
    let mut orbit_state = OrbitState::new(orbit_speed);
    let mut model_spin_enabled = rotates_without_animation;
    let mut zoom = 1.0_f32;
    let mut focus_offset = Vec3::ZERO;
    let mut camera_height_offset = 0.0_f32;
    let mut center_lock_enabled = config.center_lock;
    let center_lock_mode = config.center_lock_mode;
    let mut stage_level = config.stage_level.min(4);
    let mut color_mode = config.color_mode;
    let mut braille_profile = config.braille_profile;
    let mut cinematic_mode = config.cinematic_camera;
    let camera_focus_mode = config.camera_focus;
    let mut reactive_gain = config.reactive_gain.clamp(0.0, 1.0);
    let mut exposure_bias = config.exposure_bias.clamp(-0.5, 0.8);
    let mut sync_offset_ms =
        initial_sync_offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
    let mut contrast_preset = RuntimeContrastPreset::from_profile(config.contrast_profile);
    let mut reactive_state = ReactiveState::default();
    let mut camera_director = CameraDirectorState::default();
    let mut adaptive_quality = RuntimeAdaptiveQuality::new(config.perf_profile);
    let mut visibility_watchdog = VisibilityWatchdog::default();
    let mut center_lock_state = CenterLockState::default();
    let mut auto_radius_guard = AutoRadiusGuard::default();
    let base_triangle_stride = config.triangle_stride.max(1);
    let base_min_triangle_area_px2 = config.min_triangle_area_px2.max(0.0);
    let mut io_failure_count: u8 = 0;
    let mut last_osd_notice: Option<String> = None;
    let mut osd_until: Option<Instant> = Some(Instant::now() + Duration::from_secs(2));
    let mut last_render_stats = RenderStats::default();
    if scaled {
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }

    let start = Instant::now();
    let mut prev_wall_seconds = 0.0_f32;
    let frame_budget = if config.fps_cap == 0 {
        None
    } else {
        Some(Duration::from_secs_f32(1.0 / (config.fps_cap as f32)))
    };
    let fixed_step = 1.0 / 120.0_f32;
    let mut sim_time = 0.0_f32;
    let mut sim_accum = 0.0_f32;
    if let Some(audio) = audio_sync.as_ref() {
        audio.playback.sink.play();
    }

    loop {
        let frame_start = Instant::now();
        let input = process_runtime_input(
            &mut frame,
            &mut orbit_state.enabled,
            &mut orbit_state.speed,
            &mut model_spin_enabled,
            &mut zoom,
            &mut focus_offset,
            &mut camera_height_offset,
            &mut center_lock_enabled,
            &mut stage_level,
            &mut sync_offset_ms,
            &mut contrast_preset,
            &mut braille_profile,
            &mut color_mode,
            &mut cinematic_mode,
            &mut reactive_gain,
            &mut exposure_bias,
        )?;
        if input.quit {
            break;
        }
        if input.resized {
            terminal.force_full_repaint();
            center_lock_state.reset();
            last_osd_notice = Some(format!("resize: {}x{}", frame.width, frame.height));
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.status_changed {
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.stage_changed {
            last_osd_notice = Some(format!("stage={}", stage_level));
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.center_lock_blocked_pan {
            last_osd_notice = Some("center-lock on: pan disabled (press f to unlock)".to_owned());
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }

        let elapsed_wall = start.elapsed().as_secs_f32();
        let dt = (elapsed_wall - prev_wall_seconds).max(0.0);
        prev_wall_seconds = elapsed_wall;
        sim_accum = (sim_accum + dt).min(0.25);
        while sim_accum >= fixed_step {
            sim_time += fixed_step;
            sim_accum -= fixed_step;
        }
        orbit_state.advance(dt);
        let sync_speed = audio_sync.as_ref().map(|s| s.speed_factor).unwrap_or(1.0);
        let elapsed_audio = audio_sync
            .as_ref()
            .map(|s| s.playback.sink.get_pos().as_secs_f32());
        let raw_energy = if matches!(config.audio_reactive, AudioReactiveMode::Off) {
            0.0
        } else {
            audio_sync
                .as_ref()
                .and_then(|sync| {
                    elapsed_audio
                        .and_then(|audio_time| sync.envelope.as_ref().map(|e| e.sample(audio_time)))
                })
                .unwrap_or(0.0)
        };
        reactive_state.energy = raw_energy;
        reactive_state.smoothed_energy += (raw_energy - reactive_state.smoothed_energy) * 0.18;
        let animation_time = if elapsed_audio.is_some() {
            compute_animation_time(elapsed_wall, elapsed_audio, sync_speed, sync_offset_ms)
        } else {
            let interpolated = sim_time + sim_accum / fixed_step * fixed_step;
            compute_animation_time(interpolated, None, sync_speed, sync_offset_ms)
        };
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
        if matches!(color_mode, ColorMode::Ansi) && !truecolor_supported {
            color_mode = ColorMode::Mono;
        }
        frame_config.cell_aspect_mode = CellAspectMode::Manual;
        frame_config.cell_aspect = effective_aspect;
        frame_config.center_lock = center_lock_enabled;
        frame_config.center_lock_mode = center_lock_mode;
        frame_config.stage_level = stage_level.min(4);
        frame_config.color_mode = color_mode;
        frame_config.braille_profile = braille_profile;
        frame_config.cinematic_camera = cinematic_mode;
        frame_config.camera_focus = camera_focus_mode;
        frame_config.reactive_gain = reactive_gain;
        apply_runtime_contrast_preset(&mut frame_config, contrast_preset);
        let reactive_multiplier = match frame_config.audio_reactive {
            AudioReactiveMode::Off => 0.0,
            AudioReactiveMode::On => 1.0,
            AudioReactiveMode::High => 1.6,
        };
        let reactive_amount =
            (reactive_state.smoothed_energy * frame_config.reactive_gain * reactive_multiplier)
                .clamp(0.0, 1.0);
        frame_config.reactive_pulse = reactive_amount;
        if reactive_multiplier > 0.0 {
            let centered = reactive_state.smoothed_energy - 0.5;
            frame_config.contrast_floor = (frame_config.contrast_floor
                + centered * 0.04 * frame_config.reactive_gain)
                .clamp(0.04, 0.32);
            frame_config.fog_scale =
                (frame_config.fog_scale * (1.0 - reactive_amount * 0.18)).clamp(0.30, 1.5);
        }
        frame_config.exposure_bias = exposure_bias;

        apply_adaptive_quality_tuning(
            &mut frame_config,
            base_triangle_stride,
            base_min_triangle_area_px2,
            adaptive_quality.lod_level,
        );

        let jitter_scale = jitter_scale_for_lod(adaptive_quality.lod_level);
        let (radius_mul, height_off, focus_y_off, angle_jitter) = update_camera_director(
            &mut camera_director,
            cinematic_mode,
            camera_focus_mode,
            elapsed_wall,
            reactive_state.smoothed_energy,
            reactive_gain,
            extent_y,
            jitter_scale,
        );
        let dynamic_center_offset = if center_lock_enabled {
            center_lock_state.update(
                &last_render_stats,
                center_lock_mode,
                frame.width,
                frame.height,
                framing.radius * zoom * radius_mul,
                extent_y,
            )
        } else {
            center_lock_state.reset();
            Vec3::ZERO
        };

        let auto_radius_shrink = auto_radius_guard.shrink_ratio;
        let camera = orbit_camera(
            orbit_state.angle + angle_jitter,
            (framing.radius * zoom * radius_mul * (1.0 - auto_radius_shrink)).clamp(0.2, 1000.0),
            (framing.camera_height + camera_height_offset + height_off).clamp(-1000.0, 1000.0),
            framing.focus
                + dynamic_center_offset
                + if center_lock_enabled {
                    Vec3::ZERO
                } else {
                    focus_offset
                }
                + Vec3::new(
                    0.0,
                    if center_lock_enabled {
                        focus_y_off.clamp(-extent_y * 0.03, extent_y * 0.03)
                    } else {
                        focus_y_off
                    },
                    0.0,
                ),
        );
        let stats = render_frame_with_backend(
            &mut frame,
            &frame_config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            pipeline.morph_weights_by_instance(),
            &glyph_ramp,
            &mut render_scratch,
            camera,
            rotation,
        );
        last_render_stats = stats;
        auto_radius_guard.update(
            stats.visible_height_ratio,
            center_lock_enabled && matches!(braille_profile, BrailleProfile::Safe),
        );

        if visibility_watchdog.observe(stats.visible_cell_ratio) {
            visibility_watchdog.reset();
            zoom = 1.0;
            focus_offset = Vec3::ZERO;
            camera_height_offset = 0.0;
            exposure_bias = (exposure_bias + 0.08).clamp(-0.5, 0.8);
            center_lock_state.reset();
            auto_radius_guard = AutoRadiusGuard::default();
            camera_director = CameraDirectorState::default();
            cinematic_mode = CinematicCameraMode::On;
            last_osd_notice = Some("visibility recover".to_owned());
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }

        let work_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        if adaptive_quality.observe(work_ms) {
            last_osd_notice = Some(format!(
                "lod={} target={:.1}ms ema={:.1}ms",
                adaptive_quality.lod_level,
                adaptive_quality.target_frame_ms,
                adaptive_quality.ema_frame_ms
            ));
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }

        if osd_until.is_some_and(|until| Instant::now() <= until) {
            let status = format_runtime_status(
                sync_offset_ms,
                sync_speed,
                effective_aspect,
                contrast_preset,
                frame_config.braille_profile,
                frame_config.color_mode,
                frame_config.cinematic_camera,
                frame_config.reactive_gain,
                frame_config.exposure_bias,
                frame_config.stage_level,
                frame_config.center_lock,
                adaptive_quality.lod_level,
                adaptive_quality.target_frame_ms,
                adaptive_quality.ema_frame_ms,
                last_osd_notice.as_deref(),
            );
            overlay_osd(&mut frame, &status);
        }

        let present_result = if matches!(frame_config.color_mode, ColorMode::Ansi) {
            terminal.present(&frame, true)
        } else {
            terminal.present(&frame, false)
        };
        if let Err(err) = present_result {
            if is_retryable_io_error(&err) {
                io_failure_count = io_failure_count.saturating_add(1);
                if io_failure_count >= 3 {
                    io_failure_count = 0;
                    color_mode = ColorMode::Mono;
                    terminal.set_present_mode(PresentMode::FullFallback);
                    last_osd_notice = Some("io fallback: mono/full".to_owned());
                    osd_until = Some(Instant::now() + Duration::from_secs(3));
                }
                continue;
            }
            io_failure_count = io_failure_count.saturating_add(1);
            if io_failure_count >= 3 {
                io_failure_count = 0;
                color_mode = ColorMode::Mono;
                terminal.set_present_mode(PresentMode::FullFallback);
                last_osd_notice = Some("error fallback: mono/full".to_owned());
                osd_until = Some(Instant::now() + Duration::from_secs(3));
                continue;
            }
            return Err(err);
        }
        io_failure_count = 0;
        if input.last_key.is_some() {
            last_osd_notice = None;
        }

        set_runtime_panic_state(format!(
            "mode={:?} backend={:?} size={}x{} fps_cap={} key={} lod={}",
            frame_config.mode,
            frame_config.backend,
            frame.width,
            frame.height,
            frame_config.fps_cap,
            input.last_key.unwrap_or("-"),
            adaptive_quality.lod_level
        ));

        let elapsed_frame = frame_start.elapsed();
        if let Some(frame_budget) = frame_budget {
            if elapsed_frame < frame_budget {
                thread::sleep(frame_budget - elapsed_frame);
            }
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
    let mode: RenderMode = args.mode.into();
    let color_mode = visual
        .color_mode
        .unwrap_or_else(|| default_color_mode_for_mode(mode));
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        braille_profile: visual.braille_profile,
        theme_style: visual.theme_style,
        audio_reactive: visual.audio_reactive,
        cinematic_camera: visual.cinematic_camera,
        camera_focus: visual.camera_focus,
        reactive_gain: visual.reactive_gain,
        reactive_pulse: 0.0,
        exposure_bias: visual.exposure_bias,
        center_lock: visual.center_lock,
        center_lock_mode: visual.center_lock_mode,
        stage_level: visual.stage_level,
        stage_reactive: visual.stage_reactive,
        material_color: visual.material_color,
        texture_sampling: visual.texture_sampling,
        braille_aspect_compensation: visual.braille_aspect_compensation,
        charset: args.charset.clone(),
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        fps_cap: args.fps_cap,
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
    let mode: RenderMode = args.mode.into();
    let color_mode = visual
        .color_mode
        .unwrap_or_else(|| default_color_mode_for_mode(mode));
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        braille_profile: visual.braille_profile,
        theme_style: visual.theme_style,
        audio_reactive: visual.audio_reactive,
        cinematic_camera: visual.cinematic_camera,
        camera_focus: visual.camera_focus,
        reactive_gain: visual.reactive_gain,
        reactive_pulse: 0.0,
        exposure_bias: visual.exposure_bias,
        center_lock: visual.center_lock,
        center_lock_mode: visual.center_lock_mode,
        stage_level: visual.stage_level,
        stage_reactive: visual.stage_reactive,
        material_color: visual.material_color,
        texture_sampling: visual.texture_sampling,
        braille_aspect_compensation: visual.braille_aspect_compensation,
        charset: args.charset.clone(),
        cell_aspect: args.cell_aspect,
        cell_aspect_mode: visual.cell_aspect_mode,
        cell_aspect_trim: visual.cell_aspect_trim,
        fps_cap: args.fps_cap,
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

fn build_audio_envelope(path: Option<&Path>, fps: u32) -> Option<AudioEnvelope> {
    let path = path?;
    if fps == 0 {
        return None;
    }
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels().max(1) as usize;
    let sample_rate = decoder.sample_rate().max(1);
    let total_duration = decoder
        .total_duration()
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let samples_per_bucket =
        ((sample_rate as f32 * channels as f32) / (fps as f32)).round() as usize;
    let bucket_size = samples_per_bucket.max(channels);

    let mut values = Vec::new();
    let mut acc = 0.0_f32;
    let mut count = 0_usize;
    for sample in decoder {
        let s = (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0);
        acc += s * s;
        count += 1;
        if count >= bucket_size {
            let rms = (acc / (count as f32)).sqrt();
            values.push(rms);
            acc = 0.0;
            count = 0;
        }
    }
    if count > 0 {
        values.push((acc / (count as f32)).sqrt());
    }
    if values.is_empty() {
        return None;
    }

    let max = values
        .iter()
        .copied()
        .fold(0.0_f32, |a, b| if b > a { b } else { a });
    if max > f32::EPSILON {
        for value in &mut values {
            *value = (*value / max).clamp(0.0, 1.0);
        }
    }
    let duration_secs = if total_duration > f32::EPSILON {
        total_duration
    } else {
        (values.len() as f32) / (fps as f32)
    };
    Some(AudioEnvelope {
        fps,
        values,
        duration_secs,
    })
}

fn prepare_audio_sync(
    music_path: Option<&Path>,
    clip_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> Option<AudioSyncRuntime> {
    let envelope = build_audio_envelope(music_path, 60);
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
        envelope,
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
    braille_profile: BrailleProfile,
    color_mode: ColorMode,
    cinematic_mode: CinematicCameraMode,
    reactive_gain: f32,
    exposure_bias: f32,
    stage_level: u8,
    center_lock: bool,
    lod_level: usize,
    target_ms: f32,
    frame_ema_ms: f32,
    notice: Option<&str>,
) -> String {
    let core = format!(
        "offset={sync_offset_ms}ms  speed={sync_speed:.4}x  aspect={effective_aspect:.3}  contrast={}  braille={:?}  color={:?}  camera={:?}  gain={reactive_gain:.2}  exp={exposure_bias:+.2}  stage={}  center={}  lod={}  target={target_ms:.1}ms  ema={frame_ema_ms:.1}ms",
        contrast.label(),
        braille_profile,
        color_mode,
        cinematic_mode,
        stage_level,
        if center_lock { "on" } else { "off" },
        lod_level
    );
    if let Some(extra) = notice {
        format!("{core}  note={extra}")
    } else {
        core
    }
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
    for color in &mut frame.fg_rgb[row_start..row_end] {
        *color = [235, 235, 235];
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
    center_lock_enabled: &mut bool,
    stage_level: &mut u8,
    sync_offset_ms: &mut i32,
    contrast_preset: &mut RuntimeContrastPreset,
    braille_profile: &mut BrailleProfile,
    color_mode: &mut ColorMode,
    cinematic_mode: &mut CinematicCameraMode,
    reactive_gain: &mut f32,
    exposure_bias: &mut f32,
) -> Result<RuntimeInputResult> {
    let mut result = RuntimeInputResult::default();
    while event::poll(Duration::from_millis(0))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    result.quit = true;
                    result.last_key = Some("q");
                    return Ok(result);
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    *orbit_enabled = !*orbit_enabled;
                    result.last_key = Some("o");
                    result.status_changed = true;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    *model_spin_enabled = !*model_spin_enabled;
                    result.last_key = Some("r");
                    result.status_changed = true;
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    *stage_level = stage_level.saturating_add(1).min(4);
                    result.status_changed = true;
                    result.stage_changed = true;
                    result.last_key = Some("+");
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    *stage_level = stage_level.saturating_sub(1);
                    result.status_changed = true;
                    result.stage_changed = true;
                    result.last_key = Some("-");
                }
                KeyCode::Char('e') => {
                    *exposure_bias = (*exposure_bias - 0.04).clamp(-0.5, 0.8);
                    result.status_changed = true;
                    result.last_key = Some("e");
                }
                KeyCode::Char('E') => {
                    *exposure_bias = (*exposure_bias + 0.04).clamp(-0.5, 0.8);
                    result.status_changed = true;
                    result.last_key = Some("E");
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    *center_lock_enabled = !*center_lock_enabled;
                    result.status_changed = true;
                    result.last_key = Some("f");
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    *orbit_speed = (*orbit_speed + 0.05).clamp(0.0, 3.0);
                    if *orbit_speed > 0.0 {
                        *orbit_enabled = true;
                    }
                    result.status_changed = true;
                    result.last_key = Some("x");
                }
                KeyCode::Char('z') | KeyCode::Char('Z') => {
                    *orbit_speed = (*orbit_speed - 0.05).clamp(0.0, 3.0);
                    result.status_changed = true;
                    result.last_key = Some("z");
                }
                KeyCode::Char('[') => *zoom = (*zoom + 0.08).clamp(0.2, 8.0),
                KeyCode::Char(']') => *zoom = (*zoom - 0.08).clamp(0.2, 8.0),
                KeyCode::Left | KeyCode::Char('j') | KeyCode::Char('J') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x -= 0.08;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x += 0.08;
                    }
                }
                KeyCode::Up | KeyCode::Char('i') | KeyCode::Char('I') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.y += 0.08;
                        *camera_height_offset += 0.08;
                    }
                }
                KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('K') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.y -= 0.08;
                        *camera_height_offset -= 0.08;
                    }
                }
                KeyCode::Char('u') | KeyCode::Char('U') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.z += 0.08;
                    }
                }
                KeyCode::Char('m') | KeyCode::Char('M') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.z -= 0.08;
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    *zoom = 1.0;
                    *focus_offset = Vec3::ZERO;
                    *camera_height_offset = 0.0;
                    result.status_changed = true;
                    result.last_key = Some("c");
                }
                KeyCode::Char(',') => {
                    *sync_offset_ms = (*sync_offset_ms - SYNC_OFFSET_STEP_MS)
                        .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
                    result.status_changed = true;
                    result.last_key = Some(",");
                }
                KeyCode::Char('.') => {
                    *sync_offset_ms = (*sync_offset_ms + SYNC_OFFSET_STEP_MS)
                        .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
                    result.status_changed = true;
                    result.last_key = Some(".");
                }
                KeyCode::Char('/') => {
                    *sync_offset_ms = 0;
                    result.status_changed = true;
                    result.last_key = Some("/");
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    *contrast_preset = contrast_preset.next();
                    result.status_changed = true;
                    result.last_key = Some("v");
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    *braille_profile = match *braille_profile {
                        BrailleProfile::Safe => BrailleProfile::Normal,
                        BrailleProfile::Normal => BrailleProfile::Dense,
                        BrailleProfile::Dense => BrailleProfile::Safe,
                    };
                    result.status_changed = true;
                    result.last_key = Some("b");
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    *color_mode = match *color_mode {
                        ColorMode::Mono => ColorMode::Ansi,
                        ColorMode::Ansi => ColorMode::Mono,
                    };
                    result.status_changed = true;
                    result.last_key = Some("n");
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    *cinematic_mode = match *cinematic_mode {
                        CinematicCameraMode::Off => CinematicCameraMode::On,
                        _ => CinematicCameraMode::Off,
                    };
                    result.status_changed = true;
                    result.last_key = Some("p");
                }
                KeyCode::Char('g') => {
                    *reactive_gain = (*reactive_gain - 0.05).clamp(0.0, 1.0);
                    result.status_changed = true;
                    result.last_key = Some("g");
                }
                KeyCode::Char('G') => {
                    *reactive_gain = (*reactive_gain + 0.05).clamp(0.0, 1.0);
                    result.status_changed = true;
                    result.last_key = Some("G");
                }
                _ => {}
            },
            Event::Resize(width, height) => {
                let (rw, rh, _) = cap_render_size(width, height);
                frame.resize(rw.max(1), rh.max(1));
                result.status_changed = true;
                result.resized = true;
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
    let mode: RenderMode = args.mode.into();
    let color_mode = visual
        .color_mode
        .unwrap_or_else(|| default_color_mode_for_mode(mode));
    let mut config = RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        braille_profile: visual.braille_profile,
        theme_style: visual.theme_style,
        audio_reactive: visual.audio_reactive,
        cinematic_camera: visual.cinematic_camera,
        camera_focus: visual.camera_focus,
        reactive_gain: visual.reactive_gain,
        reactive_pulse: 0.0,
        exposure_bias: visual.exposure_bias,
        center_lock: visual.center_lock,
        center_lock_mode: visual.center_lock_mode,
        stage_level: visual.stage_level,
        stage_reactive: visual.stage_reactive,
        material_color: visual.material_color,
        texture_sampling: visual.texture_sampling,
        braille_aspect_compensation: visual.braille_aspect_compensation,
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
    config.backend = resolve_runtime_backend(config.backend);
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
        let stats = render_frame_with_backend(
            &mut frame,
            &config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            pipeline.morph_weights_by_instance(),
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
    println!("materials: {}", scene.materials.len());
    println!("textures: {}", scene.textures.len());
    println!("animations: {}", scene.animations.len());
    let total_morph_targets: usize = scene
        .meshes
        .iter()
        .map(|mesh| mesh.morph_targets.len())
        .sum();
    let weighted_instances = scene
        .mesh_instances
        .iter()
        .filter(|instance| !instance.default_morph_weights.is_empty())
        .count();
    println!("morph_targets: {}", total_morph_targets);
    println!("morph_weighted_instances: {}", weighted_instances);
    let vertex_color_primitives = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.colors_rgba.as_ref().is_some_and(|c| !c.is_empty()))
        .count();
    let uv_primitives = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.uv0.as_ref().is_some_and(|u| !u.is_empty()))
        .count();
    println!("vertex_color_primitives: {}", vertex_color_primitives);
    println!("uv_primitives: {}", uv_primitives);
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
        let mut t_count = 0usize;
        let mut r_count = 0usize;
        let mut s_count = 0usize;
        let mut m_count = 0usize;
        for channel in &animation.channels {
            match channel.target {
                ChannelTarget::Translation => t_count += 1,
                ChannelTarget::Rotation => r_count += 1,
                ChannelTarget::Scale => s_count += 1,
                ChannelTarget::MorphWeights => m_count += 1,
            }
        }
        println!(
            "animation[{index}]: name={} duration={:.3}s channels={} (t/r/s/m={}/{}/{}/{})",
            animation.name.as_deref().unwrap_or("<unnamed>"),
            animation.duration,
            animation.channels.len(),
            t_count,
            r_count,
            s_count,
            m_count
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

fn update_camera_director(
    state: &mut CameraDirectorState,
    mode: CinematicCameraMode,
    focus_mode: CameraFocusMode,
    elapsed_wall: f32,
    smoothed_energy: f32,
    reactive_gain: f32,
    extent_y: f32,
    jitter_scale: f32,
) -> (f32, f32, f32, f32) {
    if matches!(mode, CinematicCameraMode::Off) {
        return camera_shot_values(CameraShot::FullBody, extent_y);
    }
    if !matches!(focus_mode, CameraFocusMode::Auto) {
        let shot = match focus_mode {
            CameraFocusMode::Auto | CameraFocusMode::Full => CameraShot::FullBody,
            CameraFocusMode::Upper => CameraShot::UpperBody,
            CameraFocusMode::Face => CameraShot::FaceCloseup,
            CameraFocusMode::Hands => CameraShot::Hands,
        };
        return camera_shot_values(shot, extent_y);
    }

    let dt = (elapsed_wall - state.total_time_accum).max(0.0);
    state.total_time_accum = elapsed_wall;
    if matches!(state.shot, CameraShot::FaceCloseup) {
        state.face_time_accum += dt;
    }

    let mut should_cut = elapsed_wall >= state.next_cut_at;
    let face_ratio = if state.total_time_accum > 0.0 {
        state.face_time_accum / state.total_time_accum
    } else {
        0.0
    };
    if smoothed_energy > 0.72 && (elapsed_wall - state.transition_started_at) > 2.5 {
        should_cut = true;
    }
    if should_cut {
        let next_shot = match state.shot {
            CameraShot::FullBody => CameraShot::UpperBody,
            CameraShot::UpperBody => {
                if face_ratio < 0.25 {
                    CameraShot::FaceCloseup
                } else {
                    CameraShot::FullBody
                }
            }
            CameraShot::FaceCloseup => CameraShot::Hands,
            CameraShot::Hands => CameraShot::FullBody,
        };
        state.shot = next_shot;
        state.transition_started_at = elapsed_wall;
        state.previous_radius_mul = state.radius_mul;
        state.previous_height_offset = state.height_offset;
        state.previous_focus_y_offset = state.focus_y_offset;
        let (radius_mul, height_off, focus_y_off, base_duration) = match state.shot {
            CameraShot::FullBody => (1.0, 0.0, 0.0, 6.0),
            CameraShot::UpperBody => (0.66, extent_y * 0.08, extent_y * 0.16, 5.0),
            CameraShot::FaceCloseup => (0.42, extent_y * 0.26, extent_y * 0.39, 3.0),
            CameraShot::Hands => (0.52, extent_y * 0.04, extent_y * 0.12, 3.8),
        };
        state.radius_mul = radius_mul;
        state.height_offset = height_off;
        state.focus_y_offset = focus_y_off;
        let energy_advance = (smoothed_energy * 1.6).clamp(0.0, 1.0);
        state.next_cut_at = elapsed_wall + (base_duration - energy_advance).clamp(2.2, 8.0);
    }

    let transition_t = ((elapsed_wall - state.transition_started_at) / 0.35).clamp(0.0, 1.0);
    let eased_t = transition_t * transition_t * (3.0 - 2.0 * transition_t);
    let radius_mul =
        state.previous_radius_mul + (state.radius_mul - state.previous_radius_mul) * eased_t;
    let height_off = state.previous_height_offset
        + (state.height_offset - state.previous_height_offset) * eased_t;
    let focus_y_off = state.previous_focus_y_offset
        + (state.focus_y_offset - state.previous_focus_y_offset) * eased_t;

    state.jitter_phase += 0.09;
    let jitter_gain = match mode {
        CinematicCameraMode::On => 1.0,
        CinematicCameraMode::Aggressive => 1.7,
        CinematicCameraMode::Off => 0.0,
    };
    let jitter = (state.jitter_phase * 0.8).sin()
        * 0.015
        * smoothed_energy
        * reactive_gain
        * jitter_gain
        * jitter_scale;
    (radius_mul, height_off, focus_y_off, jitter)
}

fn camera_shot_values(shot: CameraShot, extent_y: f32) -> (f32, f32, f32, f32) {
    match shot {
        CameraShot::FullBody => (1.0, 0.0, 0.0, 0.0),
        CameraShot::UpperBody => (0.66, extent_y * 0.08, extent_y * 0.16, 0.0),
        CameraShot::FaceCloseup => (0.42, extent_y * 0.26, extent_y * 0.39, 0.0),
        CameraShot::Hands => (0.52, extent_y * 0.04, extent_y * 0.12, 0.0),
    }
}

fn orbit_camera(orbit_angle: f32, orbit_radius: f32, camera_height: f32, focus: Vec3) -> Camera {
    let eye_x = focus.x + orbit_angle.cos() * orbit_radius;
    let eye_z = focus.z + orbit_angle.sin() * orbit_radius;
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
        let (w, h, scaled) = cap_render_size(1000, 500);
        assert!(scaled);
        assert!(w <= MAX_RENDER_COLS);
        assert!(h <= MAX_RENDER_ROWS);
    }
}
