use std::{
    collections::BTreeMap,
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
use glam::{Quat, Vec3};
use rodio::{Decoder, OutputStream, Sink, Source};

use crate::{
    animation::{ChannelTarget, compute_global_matrices, default_poses},
    assets::vmd_camera::parse_vmd_camera,
    cli::{
        BenchArgs, BenchSceneArg, Cli, Commands, InspectArgs, PreprocessArgs, PreviewArgs, RunArgs,
        RunSceneArg, StartArgs,
    },
    engine::camera_track::{CameraTrackSampler, MmdCameraTransform},
    loader,
    pipeline::FramePipeline,
    render::backend::render_frame_with_backend,
    renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, RenderStats},
    runtime::{
        config::{GasciiConfig, load_gascii_config},
        graphics_proto::{
            cleanup_orphan_shm_files, cleanup_shm_registry, detect_supported_protocol,
        },
        preprocess::run_preprocess,
        preview::run_preview_server,
        start_ui::{
            StageChoice, StageStatus, StageTransform, StartWizardDefaults, run_start_wizard,
        },
        sync_profile::{
            SyncProfileEntry, SyncProfileMode, SyncProfileStore, build_profile_key,
            default_profile_store_path,
        },
    },
    scene::{
        AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
        CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
        ClarityProfile, ColorMode, ContrastProfile, DetailProfile, FreeFlyState, GraphicsProtocol,
        KittyCompression, KittyInternalResPreset, KittyPipelineMode, KittyTransport, MeshLayer,
        Node, PerfProfile, RecoverStrategy, RenderBackend, RenderConfig, RenderMode,
        RenderOutputMode, SceneCpu, StageRole, SyncPolicy, SyncSpeedMode, TextureSamplingMode,
        ThemeStyle, estimate_cell_aspect_from_window, kitty_internal_resolution,
        resolve_cell_aspect,
    },
    terminal::{PresentMode, TerminalProfile, TerminalSession},
};

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

const SYNC_OFFSET_STEP_MS: i32 = 10;
const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;
const MAX_RENDER_COLS: u16 = 4096;
const MAX_RENDER_ROWS: u16 = 2048;
const VISIBILITY_LOW_THRESHOLD: f32 = 0.002;
const VISIBILITY_LOW_FRAMES_TO_RECOVER: u32 = 12;
const LOW_VIS_EXPOSURE_THRESHOLD: f32 = 0.008;
const LOW_VIS_EXPOSURE_TRIGGER_FRAMES: u32 = 6;
const LOW_VIS_EXPOSURE_RECOVER_THRESHOLD: f32 = 0.020;
const LOW_VIS_EXPOSURE_RECOVER_FRAMES: u32 = 24;
const MIN_VISIBLE_HEIGHT_RATIO: f32 = 0.10;
const MIN_VISIBLE_HEIGHT_TRIGGER_FRAMES: u32 = 10;
const MIN_VISIBLE_HEIGHT_RECOVER_RATIO: f32 = 0.16;
const MIN_VISIBLE_HEIGHT_RECOVER_FRAMES: u32 = 30;
const HYBRID_GRAPHICS_MAX_CELLS: usize = 24_000;
const HYBRID_GRAPHICS_SLOW_FRAME_MS: f32 = 45.0;
const HYBRID_GRAPHICS_SLOW_STREAK_LIMIT: u32 = 8;

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

#[derive(Debug, Default, Clone, Copy)]
struct ContinuousSyncState {
    anim_time: f32,
    initialized: bool,
    drift_ema: f32,
    hard_snap_count: u32,
}

#[derive(Debug, Clone)]
struct RuntimeCameraSettings {
    mode: CameraMode,
    align_preset: CameraAlignPreset,
    unit_scale: f32,
    vmd_fps: f32,
    vmd_path: Option<PathBuf>,
    look_speed: f32,
}

#[derive(Debug, Clone)]
struct LoadedCameraTrack {
    sampler: CameraTrackSampler,
    transform: MmdCameraTransform,
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
    terminal_size_unstable: bool,
    resized_terminal: Option<(u16, u16)>,
    stage_changed: bool,
    center_lock_blocked_pan: bool,
    center_lock_auto_disabled: bool,
    freefly_toggled: bool,
    zoom_changed: bool,
    last_key: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeCameraState {
    control_mode: CameraControlMode,
    previous_control_mode: CameraControlMode,
    track_enabled: bool,
    active_track_mode: CameraMode,
    saved_track_mode: CameraMode,
}

impl RuntimeCameraState {
    fn new(
        control_mode: CameraControlMode,
        track_mode: CameraMode,
        has_track_source: bool,
    ) -> Self {
        let track_capable = has_track_source && !matches!(track_mode, CameraMode::Off);
        let effective_control_mode = if track_capable {
            CameraControlMode::Orbit
        } else {
            control_mode
        };
        Self {
            control_mode: effective_control_mode,
            previous_control_mode: effective_control_mode,
            track_enabled: track_capable,
            active_track_mode: track_mode,
            saved_track_mode: track_mode,
        }
    }

    fn toggle_freefly(&mut self, has_track_source: bool) -> bool {
        if !matches!(self.control_mode, CameraControlMode::FreeFly) {
            self.previous_control_mode = self.control_mode;
            self.control_mode = CameraControlMode::FreeFly;
            if self.track_enabled {
                self.saved_track_mode = self.active_track_mode;
            }
            self.track_enabled = false;
            true
        } else {
            self.control_mode = if matches!(self.previous_control_mode, CameraControlMode::FreeFly)
            {
                CameraControlMode::Orbit
            } else {
                self.previous_control_mode
            };
            self.active_track_mode = self.saved_track_mode;
            self.track_enabled =
                has_track_source && !matches!(self.active_track_mode, CameraMode::Off);
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorPathLevel {
    Truecolor,
    Q216,
    Mono,
}

#[derive(Debug, Clone, Copy)]
struct ColorRecoveryState {
    level: ColorPathLevel,
    target_level: ColorPathLevel,
    auto_recover: bool,
    success_streak: u32,
}

impl ColorRecoveryState {
    fn from_requested(
        requested_color: ColorMode,
        requested_quantization: AnsiQuantization,
        auto_recover: bool,
    ) -> Self {
        let target_level = if matches!(requested_color, ColorMode::Mono) {
            ColorPathLevel::Mono
        } else if matches!(requested_quantization, AnsiQuantization::Off) {
            ColorPathLevel::Truecolor
        } else {
            ColorPathLevel::Q216
        };
        Self {
            level: target_level,
            target_level,
            auto_recover,
            success_streak: 0,
        }
    }

    fn set_requested(
        &mut self,
        requested_color: ColorMode,
        requested_quantization: AnsiQuantization,
    ) {
        self.target_level = if matches!(requested_color, ColorMode::Mono) {
            ColorPathLevel::Mono
        } else if matches!(requested_quantization, AnsiQuantization::Off) {
            ColorPathLevel::Truecolor
        } else {
            ColorPathLevel::Q216
        };
        self.level = self.target_level;
        self.success_streak = 0;
    }

    fn degrade(&mut self, ascii_force_color_active: bool, mode: RenderMode) -> bool {
        self.success_streak = 0;
        let previous = self.level;
        self.level = match self.level {
            ColorPathLevel::Truecolor => ColorPathLevel::Q216,
            ColorPathLevel::Q216 => {
                if matches!(mode, RenderMode::Ascii) && ascii_force_color_active {
                    ColorPathLevel::Q216
                } else {
                    ColorPathLevel::Mono
                }
            }
            ColorPathLevel::Mono => ColorPathLevel::Mono,
        };
        self.level != previous
    }

    fn on_present_success(&mut self) -> bool {
        if !self.auto_recover {
            self.success_streak = 0;
            return false;
        }
        if self.level == self.target_level {
            self.success_streak = 0;
            return false;
        }
        self.success_streak = self.success_streak.saturating_add(1);
        let threshold = match self.level {
            ColorPathLevel::Mono => 150,
            ColorPathLevel::Q216 => 210,
            ColorPathLevel::Truecolor => u32::MAX,
        };
        if self.success_streak < threshold {
            return false;
        }
        self.success_streak = 0;
        self.level = match self.level {
            ColorPathLevel::Mono => ColorPathLevel::Q216,
            ColorPathLevel::Q216 => ColorPathLevel::Truecolor,
            ColorPathLevel::Truecolor => ColorPathLevel::Truecolor,
        };
        true
    }

    fn apply(
        &self,
        color_mode: &mut ColorMode,
        quantization: &mut AnsiQuantization,
        mode: RenderMode,
        ascii_force_color_active: bool,
    ) {
        match self.level {
            ColorPathLevel::Truecolor => {
                *color_mode = ColorMode::Ansi;
                *quantization = AnsiQuantization::Off;
            }
            ColorPathLevel::Q216 => {
                *color_mode = ColorMode::Ansi;
                *quantization = AnsiQuantization::Q216;
            }
            ColorPathLevel::Mono => {
                if matches!(mode, RenderMode::Ascii) && ascii_force_color_active {
                    *color_mode = ColorMode::Ansi;
                    *quantization = AnsiQuantization::Q216;
                } else {
                    *color_mode = ColorMode::Mono;
                }
            }
        }
    }
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
}

impl CenterLockState {
    fn apply_camera_space(
        &mut self,
        stats: &RenderStats,
        mode: CenterLockMode,
        frame_width: u16,
        frame_height: u16,
        camera: &mut Camera,
        fov_deg: f32,
        cell_aspect: f32,
        extent_y: f32,
    ) {
        let fw = f32::from(frame_width.max(1));
        let fh = f32::from(frame_height.max(1));
        let root_in_view = stats.root_screen_px.filter(|(x, y)| {
            x.is_finite() && y.is_finite() && *x >= 0.0 && *x <= fw && *y >= 0.0 && *y <= fh
        });
        let anchor = match mode {
            CenterLockMode::Root => stats
                .subject_centroid_px
                .or(root_in_view)
                .or(stats.visible_centroid_px),
            CenterLockMode::Mixed => match (
                root_in_view,
                stats.subject_centroid_px.or(stats.visible_centroid_px),
            ) {
                (Some(root), Some(centroid)) => Some((
                    root.0 * 0.7 + centroid.0 * 0.3,
                    root.1 * 0.7 + centroid.1 * 0.3,
                )),
                (Some(root), None) => Some(root),
                (None, Some(centroid)) => Some(centroid),
                (None, None) => root_in_view,
            },
        };
        let Some((cx, cy)) = anchor else {
            self.err_x_ema *= 0.85;
            self.err_y_ema *= 0.85;
            return;
        };

        // Ignore stale or out-of-range anchors (common during terminal resize transitions).
        if cx < -fw * 0.25 || cx > fw * 1.25 || cy < -fh * 0.25 || cy > fh * 1.25 {
            self.err_x_ema *= 0.85;
            self.err_y_ema *= 0.85;
            return;
        }
        let nx = ((cx / fw - 0.5) * 2.0).clamp(-1.0, 1.0);
        let ny = ((cy / fh - 0.5) * 2.0).clamp(-1.0, 1.0);
        let dead_x = if nx.abs() < 0.015 { 0.0 } else { nx };
        let dead_y = if ny.abs() < 0.020 { 0.0 } else { ny };

        let large_error = dead_x.abs() > 0.35 || dead_y.abs() > 0.35;
        if large_error {
            self.err_x_ema = dead_x;
            self.err_y_ema = dead_y;
        } else {
            self.err_x_ema += (dead_x - self.err_x_ema) * 0.28;
            self.err_y_ema += (dead_y - self.err_y_ema) * 0.28;
        }

        let extent = extent_y.max(0.5);
        let mut forward = camera.target - camera.eye;
        if forward.length_squared() <= f32::EPSILON {
            return;
        }
        forward = forward.normalize();
        let mut right = forward.cross(camera.up);
        if right.length_squared() <= f32::EPSILON {
            return;
        }
        right = right.normalize();
        let mut up = right.cross(forward);
        if up.length_squared() <= f32::EPSILON {
            return;
        }
        up = up.normalize();

        let dist = (camera.target - camera.eye).length().max(0.2);
        let fov_y = fov_deg.to_radians().clamp(0.35, 2.6);
        let aspect = ((fw * cell_aspect.max(0.15)).max(1.0) / fh.max(1.0)).clamp(0.3, 5.0);
        let tan_y = (fov_y * 0.5).tan().max(0.01);
        let fov_x = 2.0 * (tan_y * aspect).atan();
        let tan_x = (fov_x * 0.5).tan().max(0.01);
        let shift_x = (self.err_x_ema * dist * tan_x * 0.95).clamp(-extent * 0.9, extent * 0.9);
        let shift_y = (-self.err_y_ema * dist * tan_y * 0.95).clamp(-extent * 0.75, extent * 0.75);
        let shift = right * shift_x + up * shift_y;
        camera.eye += shift;
        camera.target += shift;
    }

    fn reset(&mut self) {
        self.err_x_ema = 0.0;
        self.err_y_ema = 0.0;
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenFitController {
    auto_zoom_gain: f32,
}

impl Default for ScreenFitController {
    fn default() -> Self {
        Self {
            auto_zoom_gain: 1.0,
        }
    }
}

impl ScreenFitController {
    fn on_resize(&mut self) {
        self.auto_zoom_gain = 1.0;
    }

    fn on_manual_zoom(&mut self) {
        self.auto_zoom_gain = self.auto_zoom_gain.clamp(0.55, 1.80);
    }

    fn target_for_mode(mode: RenderMode) -> f32 {
        match mode {
            RenderMode::Ascii => 0.72,
            RenderMode::Braille => 0.66,
        }
    }

    fn update(&mut self, visible_height_ratio: f32, mode: RenderMode, enabled: bool) {
        if !enabled {
            self.auto_zoom_gain = 1.0;
            return;
        }
        if !visible_height_ratio.is_finite() || visible_height_ratio <= 0.0 {
            return;
        }
        let target = Self::target_for_mode(mode);
        let err = target - visible_height_ratio;
        if err.abs() <= 0.02 {
            return;
        }
        let factor = (1.0 + err * 0.22).clamp(0.90, 1.10);
        self.auto_zoom_gain = (self.auto_zoom_gain * factor).clamp(0.55, 1.80);
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ExposureAutoBoost {
    low_streak: u32,
    high_streak: u32,
    boost: f32,
}

impl ExposureAutoBoost {
    fn on_resize(&mut self) {
        self.low_streak = 0;
        self.high_streak = 0;
        self.boost = 0.0;
    }

    fn update(&mut self, visible_ratio: f32) {
        if visible_ratio < LOW_VIS_EXPOSURE_THRESHOLD {
            self.low_streak = self.low_streak.saturating_add(1);
            self.high_streak = 0;
            if self.low_streak >= LOW_VIS_EXPOSURE_TRIGGER_FRAMES {
                self.boost = (self.boost + 0.06).clamp(0.0, 0.45);
                self.low_streak = 0;
            }
            return;
        }

        if visible_ratio > LOW_VIS_EXPOSURE_RECOVER_THRESHOLD {
            self.high_streak = self.high_streak.saturating_add(1);
            self.low_streak = 0;
            if self.high_streak >= LOW_VIS_EXPOSURE_RECOVER_FRAMES {
                self.boost = (self.boost - 0.03).clamp(0.0, 0.45);
                self.high_streak = 0;
            }
            return;
        }

        self.low_streak = 0;
        self.high_streak = 0;
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

#[derive(Debug, Default, Clone, Copy)]
struct DistanceClampGuard {
    last_eye: Option<Vec3>,
}

impl DistanceClampGuard {
    fn apply(
        &mut self,
        camera: &mut Camera,
        subject_target: Vec3,
        extent_y: f32,
        alpha: f32,
    ) -> f32 {
        let min_dist = (extent_y * 0.42).clamp(0.35, 1.20);
        let to_eye = camera.eye - subject_target;
        let dist = to_eye.length();
        let mut desired_eye = camera.eye;
        if dist < min_dist {
            let dir = if dist <= f32::EPSILON {
                Vec3::new(0.0, 0.0, 1.0)
            } else {
                to_eye / dist
            };
            desired_eye = subject_target + dir * min_dist;
        }
        let base_eye = self.last_eye.unwrap_or(camera.eye);
        let a = alpha.clamp(0.0, 1.0);
        camera.eye = base_eye + (desired_eye - base_eye) * a;
        self.last_eye = Some(camera.eye);
        min_dist
    }

    fn reset(&mut self) {
        self.last_eye = None;
    }
}

fn dynamic_clip_planes(
    min_dist: f32,
    extent_y: f32,
    camera_dist: f32,
    has_stage: bool,
) -> (f32, f32) {
    let near = (min_dist * 0.06).clamp(0.015, 0.10);
    let subject_far = min_dist + extent_y * 6.0;
    let far_target = if has_stage {
        subject_far.max(camera_dist + extent_y * 16.0)
    } else {
        subject_far
    };
    let far = far_target.clamp(near + 3.0, 500.0);
    (near, far)
}

fn start(args: StartArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let visual = resolve_visual_options_for_start(&args, &runtime_cfg);
    let sync_defaults = resolve_sync_options_for_start(&args, &runtime_cfg);
    let sync_profile_defaults = resolve_sync_profile_options_for_start(&args, &runtime_cfg);
    let model_files = discover_glb_files(&args.dir)?;
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
        .or_else(|| resolve_camera_vmd_selector(&camera_files, selector))
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
        &args.music_dir,
        &stage_dir,
        &camera_dir,
        &model_files,
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
    let mut scene = loader::load_gltf(&selection.glb_path)?;
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
    let (sync_profile_context, sync_profile_entry) = resolve_sync_profile_for_assets(
        &sync_profile_defaults,
        RunSceneArg::Glb,
        Some(selection.glb_path.as_path()),
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
        .or_else(|| resolve_camera_vmd_selector(&camera_files, camera_selector))
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
                resolve_camera_vmd_selector(&camera_files, &runtime_cfg.camera_selection)
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

#[derive(Debug, Clone)]
struct ResolvedVisualOptions {
    output_mode: RenderOutputMode,
    recover_color_auto: bool,
    graphics_protocol: GraphicsProtocol,
    kitty_transport: KittyTransport,
    kitty_compression: KittyCompression,
    kitty_internal_res: KittyInternalResPreset,
    kitty_pipeline_mode: KittyPipelineMode,
    recover_strategy: RecoverStrategy,
    kitty_scale: f32,
    hq_target_fps: u32,
    subject_exposure_only: bool,
    subject_target_height_ratio: f32,
    subject_target_width_ratio: f32,
    quality_auto_distance: bool,
    texture_mip_bias: f32,
    stage_as_sub_only: bool,
    stage_role: StageRole,
    stage_luma_cap: f32,
    cell_aspect_mode: CellAspectMode,
    cell_aspect_trim: f32,
    contrast_profile: ContrastProfile,
    perf_profile: PerfProfile,
    detail_profile: DetailProfile,
    backend: RenderBackend,
    exposure_bias: f32,
    center_lock: bool,
    center_lock_mode: CenterLockMode,
    wasd_mode: CameraControlMode,
    freefly_speed: f32,
    camera_look_speed: f32,
    camera_mode: CameraMode,
    camera_align_preset: CameraAlignPreset,
    camera_unit_scale: f32,
    camera_vmd_fps: f32,
    camera_vmd_path: Option<PathBuf>,
    camera_focus: CameraFocusMode,
    material_color: bool,
    texture_sampling: TextureSamplingMode,
    texture_v_origin: crate::scene::TextureVOrigin,
    texture_sampler: crate::scene::TextureSamplerMode,
    clarity_profile: ClarityProfile,
    ansi_quantization: AnsiQuantization,
    model_lift: f32,
    edge_accent_strength: f32,
    bg_suppression: f32,
    braille_aspect_compensation: f32,
    stage_level: u8,
    stage_reactive: bool,
    color_mode: Option<ColorMode>,
    ascii_force_color: bool,
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
    sync_policy: SyncPolicy,
    sync_hard_snap_ms: u32,
    sync_kp: f32,
}

#[derive(Debug, Clone)]
struct ResolvedSyncProfileOptions {
    mode: SyncProfileMode,
    profile_dir: PathBuf,
    key_override: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeSyncProfileContext {
    mode: SyncProfileMode,
    store_path: PathBuf,
    key: String,
    hit: bool,
}

fn resolve_visual_options_for_start(
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

fn resolve_visual_options_for_run(
    args: &RunArgs,
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

fn resolve_visual_options_for_bench(
    args: &BenchArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedVisualOptions {
    ResolvedVisualOptions {
        output_mode: args
            .output_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.output_mode),
        recover_color_auto: runtime_cfg.recover_color_auto,
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
        wasd_mode: runtime_cfg.wasd_mode,
        freefly_speed: runtime_cfg.freefly_speed.clamp(0.1, 8.0),
        camera_look_speed: runtime_cfg.camera_look_speed.clamp(0.1, 8.0),
        camera_mode: runtime_cfg.camera_mode,
        camera_align_preset: runtime_cfg.camera_align_preset,
        camera_unit_scale: runtime_cfg.camera_unit_scale.clamp(0.01, 2.0),
        camera_vmd_fps: runtime_cfg.camera_vmd_fps.clamp(1.0, 240.0),
        camera_vmd_path: runtime_cfg.camera_vmd_path.clone(),
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

fn resolve_sync_options_for_start(
    args: &StartArgs,
    runtime_cfg: &GasciiConfig,
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
        sync_policy: args
            .sync_policy
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_policy),
        sync_hard_snap_ms: args
            .sync_hard_snap_ms
            .unwrap_or(runtime_cfg.sync_hard_snap_ms)
            .clamp(10, 2_000),
        sync_kp: args.sync_kp.unwrap_or(runtime_cfg.sync_kp).clamp(0.01, 1.0),
    }
}

fn resolve_sync_options_for_run(
    args: &RunArgs,
    runtime_cfg: &GasciiConfig,
    profile: Option<&SyncProfileEntry>,
) -> ResolvedSyncOptions {
    let profile_speed_mode = profile.and_then(|entry| entry.sync_speed_mode);
    let profile_hard_snap = profile.and_then(|entry| entry.sync_hard_snap_ms);
    let profile_kp = profile.and_then(|entry| entry.sync_kp);
    let profile_offset = profile.map(|entry| entry.sync_offset_ms);
    ResolvedSyncOptions {
        sync_offset_ms: args
            .sync_offset_ms
            .or(profile_offset)
            .unwrap_or(runtime_cfg.sync_offset_ms)
            .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
        sync_speed_mode: args
            .sync_speed_mode
            .map(Into::into)
            .or(profile_speed_mode)
            .unwrap_or(runtime_cfg.sync_speed_mode),
        sync_policy: args
            .sync_policy
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_policy),
        sync_hard_snap_ms: args
            .sync_hard_snap_ms
            .or(profile_hard_snap)
            .unwrap_or(runtime_cfg.sync_hard_snap_ms)
            .clamp(10, 2_000),
        sync_kp: args
            .sync_kp
            .or(profile_kp)
            .unwrap_or(runtime_cfg.sync_kp)
            .clamp(0.01, 1.0),
    }
}

fn resolve_sync_profile_options_for_start(
    args: &StartArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncProfileOptions {
    ResolvedSyncProfileOptions {
        mode: args
            .sync_profile_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_profile_mode),
        profile_dir: args
            .sync_profile_dir
            .clone()
            .unwrap_or_else(|| runtime_cfg.sync_profile_dir.clone()),
        key_override: args
            .sync_profile_key
            .clone()
            .filter(|value| !value.is_empty()),
    }
}

fn resolve_sync_profile_options_for_run(
    args: &RunArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncProfileOptions {
    ResolvedSyncProfileOptions {
        mode: args
            .sync_profile_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_profile_mode),
        profile_dir: args
            .sync_profile_dir
            .clone()
            .unwrap_or_else(|| runtime_cfg.sync_profile_dir.clone()),
        key_override: args
            .sync_profile_key
            .clone()
            .filter(|value| !value.is_empty()),
    }
}

fn resolve_sync_profile_for_assets(
    options: &ResolvedSyncProfileOptions,
    scene_kind: RunSceneArg,
    glb_path: Option<&Path>,
    music_path: Option<&Path>,
    camera_path: Option<&Path>,
) -> (Option<RuntimeSyncProfileContext>, Option<SyncProfileEntry>) {
    if matches!(options.mode, SyncProfileMode::Off) {
        return (None, None);
    }
    let scene_kind = match scene_kind {
        RunSceneArg::Cube => "cube",
        RunSceneArg::Obj => "obj",
        RunSceneArg::Glb => "glb",
        RunSceneArg::Pmx => "pmx",
    };
    let key = options
        .key_override
        .clone()
        .unwrap_or_else(|| build_profile_key(scene_kind, glb_path, music_path, camera_path));
    let store_path = default_profile_store_path(&options.profile_dir);
    let profile = match SyncProfileStore::load(&store_path) {
        Ok(store) => store.get(&key).cloned(),
        Err(err) => {
            eprintln!(
                "warning: failed to load sync profiles {}: {err}",
                store_path.display()
            );
            None
        }
    };
    (
        Some(RuntimeSyncProfileContext {
            mode: options.mode,
            store_path,
            key,
            hit: profile.is_some(),
        }),
        profile,
    )
}

fn default_color_mode_for_mode(mode: RenderMode) -> ColorMode {
    match mode {
        RenderMode::Braille => ColorMode::Ansi,
        RenderMode::Ascii => ColorMode::Mono,
    }
}

fn resolve_effective_color_mode(
    mode: RenderMode,
    requested: ColorMode,
    ascii_force_color: bool,
) -> ColorMode {
    if matches!(mode, RenderMode::Ascii) && ascii_force_color {
        ColorMode::Ansi
    } else {
        requested
    }
}

fn resolve_effective_camera_mode(mode: CameraMode, has_vmd_source: bool) -> CameraMode {
    if has_vmd_source && matches!(mode, CameraMode::Off) {
        CameraMode::Vmd
    } else {
        mode
    }
}

fn color_path_label(color_mode: ColorMode, quantization: AnsiQuantization) -> &'static str {
    match color_mode {
        ColorMode::Mono => "mono",
        ColorMode::Ansi => match quantization {
            AnsiQuantization::Q216 => "ansi-q216",
            AnsiQuantization::Off => "ansi-truecolor",
        },
    }
}

fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: &GasciiConfig) {
    config.triangle_stride = runtime_cfg.triangle_stride.max(1);
    config.min_triangle_area_px2 = runtime_cfg.min_triangle_area_px2.max(0.0);
    config.braille_aspect_compensation = runtime_cfg.braille_aspect_compensation;
}

fn apply_distant_subject_clarity_boost(config: &mut RenderConfig, subject_height_ratio: f32) {
    if !config.quality_auto_distance
        || !subject_height_ratio.is_finite()
        || subject_height_ratio <= 0.0
    {
        return;
    }
    let target = config.subject_target_height_ratio.clamp(0.20, 0.95);
    let distant_threshold = (target * 0.65).clamp(0.14, 0.52);
    let near_threshold = (target * 1.35).clamp(0.45, 0.98);

    if subject_height_ratio < distant_threshold {
        let t = ((distant_threshold - subject_height_ratio) / distant_threshold).clamp(0.0, 1.0);
        config.model_lift = (config.model_lift + 0.10 * t).clamp(0.02, 0.55);
        config.edge_accent_strength = (config.edge_accent_strength + 0.55 * t).clamp(0.0, 2.0);
        config.bg_suppression = (config.bg_suppression + 0.70 * t).clamp(0.0, 1.0);
        config.min_triangle_area_px2 = (config.min_triangle_area_px2 * (1.0 - 0.85 * t)).max(0.0);
        if t > 0.30 {
            config.triangle_stride = config.triangle_stride.saturating_sub(1).max(1);
        }
        if t > 0.70 {
            config.triangle_stride = config.triangle_stride.saturating_sub(1).max(1);
        }
        return;
    }

    if subject_height_ratio > near_threshold {
        let t = ((subject_height_ratio - near_threshold) / near_threshold).clamp(0.0, 1.0);
        config.edge_accent_strength = (config.edge_accent_strength * (1.0 - 0.4 * t)).max(0.05);
        config.bg_suppression = (config.bg_suppression + 0.10 * t).clamp(0.0, 1.0);
    }
}

fn apply_face_focus_detail_boost(config: &mut RenderConfig, subject_height_ratio: f32) {
    if !matches!(config.camera_focus, CameraFocusMode::Face) {
        return;
    }
    let ratio = subject_height_ratio.clamp(0.0, 1.0);
    let t = if ratio < 0.28 {
        ((0.28 - ratio) / 0.28).clamp(0.0, 1.0)
    } else {
        0.0
    };
    config.texture_mip_bias = (config.texture_mip_bias - 0.85 - 0.65 * t).clamp(-2.0, 4.0);
    config.edge_accent_strength = (config.edge_accent_strength + 0.20 + 0.30 * t).clamp(0.0, 2.0);
    config.bg_suppression = (config.bg_suppression + 0.16 + 0.22 * t).clamp(0.0, 1.0);
    if matches!(config.texture_sampling, TextureSamplingMode::Nearest) {
        config.texture_sampling = TextureSamplingMode::Bilinear;
    }
    if config.triangle_stride > 1 {
        config.triangle_stride = config.triangle_stride.saturating_sub(1);
    }
}

fn persist_sync_profile_offset(
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

fn kitty_internal_cell_size(preset: KittyInternalResPreset) -> (u16, u16) {
    let (px_w, px_h) = kitty_internal_resolution(preset);
    let cols = ((u32::from(px_w)) / 2).max(1);
    let rows = ((u32::from(px_h)) / 4).max(1);
    let capped_cols = cols.min(u32::from(u16::MAX)) as u16;
    let capped_rows = rows.min(u32::from(u16::MAX)) as u16;
    (capped_cols.max(1), capped_rows.max(1))
}

fn kitty_internal_res_level(preset: KittyInternalResPreset) -> usize {
    match preset {
        KittyInternalResPreset::R640x360 => 0,
        KittyInternalResPreset::R854x480 => 1,
        KittyInternalResPreset::R1280x720 => 2,
    }
}

fn kitty_internal_res_from_level(level: usize) -> KittyInternalResPreset {
    match level {
        0 => KittyInternalResPreset::R640x360,
        1 => KittyInternalResPreset::R854x480,
        _ => KittyInternalResPreset::R1280x720,
    }
}

fn kitty_internal_res_for_lod(
    base: KittyInternalResPreset,
    lod_level: usize,
) -> KittyInternalResPreset {
    let base_level = kitty_internal_res_level(base);
    let target_level = base_level.saturating_sub(lod_level.min(2));
    kitty_internal_res_from_level(target_level)
}

fn desired_render_cells_for_mode(
    config: &RenderConfig,
    display_cells: (u16, u16),
    graphics_enabled: bool,
) -> (u16, u16) {
    if graphics_enabled && matches!(config.output_mode, RenderOutputMode::KittyHq) {
        let (target_w, target_h) = kitty_internal_cell_size(config.kitty_internal_res);
        let (target_w, target_h, _) = cap_render_size(target_w, target_h);
        (target_w.max(1), target_h.max(1))
    } else {
        display_cells
    }
}

fn resize_runtime_frame(
    terminal: &mut TerminalSession,
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    display_cells: (u16, u16),
    graphics_enabled: bool,
) -> (u16, u16) {
    let desired = desired_render_cells_for_mode(config, display_cells, graphics_enabled);
    if frame.width != desired.0 || frame.height != desired.1 {
        frame.resize(desired.0, desired.1);
        terminal.force_full_repaint();
    }
    desired
}

fn is_terminal_size_unstable(width: u16, height: u16) -> bool {
    if width == 0 || height == 0 {
        return true;
    }
    if width == u16::MAX || height == u16::MAX {
        return true;
    }
    // Guard against transient sentinel-like resize values without rejecting legit large terminals.
    let w = width as u32;
    let h = height as u32;
    let max_w = (MAX_RENDER_COLS as u32) * 8;
    let max_h = (MAX_RENDER_ROWS as u32) * 8;
    w > max_w || h > max_h
}

fn resolve_runtime_backend(requested: RenderBackend) -> RenderBackend {
    match requested {
        RenderBackend::Cpu => RenderBackend::Cpu,
        RenderBackend::Gpu => {
            #[cfg(feature = "gpu")]
            {
                use crate::render::gpu::GpuRenderer;
                if GpuRenderer::is_available() {
                    RenderBackend::Gpu
                } else {
                    eprintln!(
                        "warning: gpu backend requested but no suitable gpu found; falling back to cpu."
                    );
                    RenderBackend::Cpu
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                eprintln!(
                    "warning: gpu backend requested but gpu feature not enabled; falling back to cpu."
                );
                RenderBackend::Cpu
            }
        }
    }
}

fn normalize_graphics_settings(config: &mut RenderConfig) -> Option<String> {
    if !matches!(
        config.output_mode,
        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
    ) {
        return None;
    }
    if matches!(config.kitty_transport, KittyTransport::Shm)
        && matches!(config.kitty_compression, KittyCompression::Zlib)
    {
        config.kitty_compression = KittyCompression::None;
        return Some("kitty transport=shm forces compression=none".to_owned());
    }
    None
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
        RunSceneArg::Pmx => {
            let path = required_path(args.pmx.as_deref(), "--pmx is required for --scene pmx")?;
            let scene = loader::load_pmx(path)?;
            Ok((scene, None, true))
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
    wasd_mode: CameraControlMode,
    freefly_speed: f32,
    camera_settings: RuntimeCameraSettings,
    sync_profile: Option<RuntimeSyncProfileContext>,
) -> Result<()> {
    config.backend = resolve_runtime_backend(config.backend);
    let startup_graphics_notice = normalize_graphics_settings(&mut config);
    let terminal_profile = TerminalProfile::detect();
    let _truecolor_supported = terminal_profile.supports_truecolor;
    let mut terminal = TerminalSession::enter_with_profile(terminal_profile)?;
    terminal.set_present_mode(PresentMode::Diff);
    let (term_width, term_height) = validated_terminal_size(&terminal)?;
    let (display_width, display_height, scaled) = cap_render_size(term_width, term_height);
    let mut display_cells = (display_width, display_height);
    let mut frame = FrameBuffers::new(display_width, display_height);
    if scaled {
        eprintln!(
            "info: terminal size {}x{} capped to internal render {}x{}",
            term_width, term_height, display_width, display_height
        );
    }
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let framing = compute_scene_framing(&scene, &config, orbit_radius, camera_height, look_at_y);
    let scene_extent_y = scene
        .mesh_instances
        .iter()
        .filter(|instance| matches!(instance.layer, MeshLayer::Subject))
        .filter_map(|instance| scene.meshes.get(instance.mesh_index))
        .flat_map(|mesh| mesh.positions.iter().map(|p| p.y))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        });
    let extent_y = if scene_extent_y.0.is_finite() && scene_extent_y.1.is_finite() {
        (scene_extent_y.1 - scene_extent_y.0).abs().max(0.5)
    } else {
        1.0
    };
    let has_stage_mesh = scene
        .mesh_instances
        .iter()
        .any(|instance| matches!(instance.layer, MeshLayer::Stage))
        && !matches!(config.stage_role, StageRole::Off);
    let mut orbit_state = OrbitState::new(orbit_speed);
    let mut model_spin_enabled = rotates_without_animation;
    let mut user_zoom = 1.0_f32;
    let mut focus_offset = Vec3::ZERO;
    let mut camera_height_offset = 0.0_f32;
    let mut center_lock_enabled = config.center_lock;
    let center_lock_mode = config.center_lock_mode;
    let mut stage_level = config.stage_level.min(4);
    let mut gpu_renderer_state = crate::render::backend_gpu::GpuRendererState::default();
    let mut requested_color_mode =
        resolve_effective_color_mode(config.mode, config.color_mode, config.ascii_force_color);
    let ascii_force_color_active = config.ascii_force_color;
    let requested_ansi_quantization = config.ansi_quantization;
    let mut color_mode = requested_color_mode;
    let mut ansi_quantization = requested_ansi_quantization;
    let mut braille_profile = config.braille_profile;
    let mut cinematic_mode = config.cinematic_camera;
    let camera_focus_mode = config.camera_focus;
    let mut reactive_gain = config.reactive_gain.clamp(0.0, 1.0);
    let mut exposure_bias = config.exposure_bias.clamp(-0.5, 0.8);
    let mut sync_offset_ms =
        initial_sync_offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
    let mut sync_profile_dirty = false;
    let mut contrast_preset = RuntimeContrastPreset::from_profile(config.contrast_profile);
    let mut reactive_state = ReactiveState::default();
    let mut camera_director = CameraDirectorState::default();
    let mut adaptive_quality = RuntimeAdaptiveQuality::new(config.perf_profile);
    let mut visibility_watchdog = VisibilityWatchdog::default();
    let mut center_lock_state = CenterLockState::default();
    let mut auto_radius_guard = AutoRadiusGuard::default();
    let mut distance_clamp_guard = DistanceClampGuard::default();
    let mut screen_fit = ScreenFitController::default();
    let mut exposure_auto_boost = ExposureAutoBoost::default();
    let base_triangle_stride = config.triangle_stride.max(1);
    let base_min_triangle_area_px2 = config.min_triangle_area_px2.max(0.0);
    let mut resize_recovery_pending = false;
    let mut center_lock_restore_after_freefly = center_lock_enabled;
    let mut io_failure_count: u8 = 0;
    let mut ghostty_zoom_repaint_due: Option<Instant> = None;
    let profile_state_hint = sync_profile.as_ref().map(|profile| {
        if profile.hit {
            "profile=hit"
        } else {
            "profile=miss"
        }
    });
    let mut last_osd_notice: Option<String> = startup_graphics_notice;
    if last_osd_notice.is_none() {
        last_osd_notice = profile_state_hint.map(str::to_owned);
    }
    let mut osd_until: Option<Instant> = Some(Instant::now() + Duration::from_secs(2));
    let mut last_render_stats = RenderStats::default();
    let mut effective_aspect_state = resolve_cell_aspect(&config, detect_terminal_cell_aspect());
    let initial_orbit_camera = orbit_camera(
        orbit_state.angle,
        framing.radius.max(0.2),
        framing.camera_height,
        framing.focus,
    );
    let mut freefly_state = freefly_state_from_camera(initial_orbit_camera, freefly_speed);
    let initial_freefly_state = freefly_state;
    let loaded_camera_track = load_camera_track(&camera_settings);
    let mut runtime_camera = RuntimeCameraState::new(
        wasd_mode,
        camera_settings.mode,
        loaded_camera_track.is_some(),
    );
    let mut color_recovery = ColorRecoveryState::from_requested(
        requested_color_mode,
        requested_ansi_quantization,
        config.recover_color_auto,
    );
    color_recovery.apply(
        &mut color_mode,
        &mut ansi_quantization,
        config.mode,
        ascii_force_color_active,
    );
    let mut active_graphics_protocol = match config.output_mode {
        RenderOutputMode::Text => None,
        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq => {
            detect_supported_protocol(config.graphics_protocol)
        }
    };
    if matches!(config.output_mode, RenderOutputMode::KittyHq) && active_graphics_protocol.is_none()
    {
        last_osd_notice = Some("kitty-hq fallback: text (protocol unsupported)".to_owned());
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }
    if matches!(config.output_mode, RenderOutputMode::Hybrid) && active_graphics_protocol.is_none()
    {
        last_osd_notice = Some("hybrid fallback: text (graphics unsupported)".to_owned());
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }
    if matches!(
        config.output_mode,
        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
    ) && active_graphics_protocol.is_some()
        && usize::from(display_cells.0).saturating_mul(usize::from(display_cells.1))
            > HYBRID_GRAPHICS_MAX_CELLS
    {
        active_graphics_protocol = None;
        last_osd_notice = Some("graphics fallback: text (terminal too large)".to_owned());
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }
    let mut render_cells = resize_runtime_frame(
        &mut terminal,
        &mut frame,
        &config,
        display_cells,
        active_graphics_protocol.is_some(),
    );
    let kitty_internal_res_base = config.kitty_internal_res;
    if matches!(
        config.output_mode,
        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
    ) && active_graphics_protocol.is_some()
    {
        terminal.force_full_repaint();
    }
    if matches!(config.output_mode, RenderOutputMode::KittyHq) && active_graphics_protocol.is_some()
    {
        last_osd_notice = Some(format!(
            "kitty-hq internal={}x{} display={}x{}",
            render_cells.0, render_cells.1, display_cells.0, display_cells.1
        ));
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }
    let clip_duration = animation_index
        .and_then(|idx| scene.animations.get(idx))
        .map(|clip| clip.duration)
        .filter(|duration| *duration > f32::EPSILON);
    let mut continuous_sync_state = ContinuousSyncState::default();
    let mut graphics_slow_streak: u32 = 0;
    let mut track_lost_streak: u32 = 0;
    let mut center_drift_streak: u32 = 0;
    if camera_settings.vmd_path.is_some() && loaded_camera_track.is_none() {
        eprintln!("warning: camera VMD could not be loaded. fallback to runtime camera.");
    }
    if scaled {
        osd_until = Some(Instant::now() + Duration::from_secs(3));
    }

    let start = Instant::now();
    let mut prev_wall_seconds = 0.0_f32;
    let frame_budget = if config.fps_cap == 0 {
        if matches!(
            config.output_mode,
            RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
        ) {
            // Graphics-protocol path is I/O heavy; keep a safety cap even in "unlimited".
            let target = config.hq_target_fps.clamp(12, 120) as f32;
            Some(Duration::from_secs_f32(1.0 / target))
        } else {
            None
        }
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
        let sync_offset_before_input = sync_offset_ms;
        let input = process_runtime_input(
            &mut orbit_state.enabled,
            &mut orbit_state.speed,
            &mut model_spin_enabled,
            &mut user_zoom,
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
            &mut runtime_camera.control_mode,
            camera_settings.look_speed,
            &mut freefly_state,
        )?;
        if input.quit {
            break;
        }
        if sync_offset_ms != sync_offset_before_input {
            sync_profile_dirty = true;
            if sync_profile.is_some() {
                last_osd_notice = Some(format!("sync profile dirty: offset={}ms", sync_offset_ms));
                osd_until = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        if input.resized {
            terminal.force_full_repaint();
            distance_clamp_guard.reset();
            screen_fit.on_resize();
            exposure_auto_boost.on_resize();
            last_render_stats = RenderStats::default();
            render_scratch.reset_exposure();
            if input.terminal_size_unstable {
                resize_recovery_pending = true;
                center_lock_state.reset();
                last_osd_notice = Some("resize unstable: waiting for terminal recovery".to_owned());
                osd_until = Some(Instant::now() + Duration::from_secs(2));
                thread::sleep(Duration::from_millis(16));
                continue;
            } else {
                resize_recovery_pending = false;
                if let Some((tw, th)) = input.resized_terminal {
                    let (rw, rh, _) = cap_render_size(tw, th);
                    display_cells = (rw.max(1), rh.max(1));
                } else if let Ok((tw, th)) = terminal.size() {
                    let (rw, rh, _) = cap_render_size(tw, th);
                    display_cells = (rw.max(1), rh.max(1));
                }
                if matches!(
                    config.output_mode,
                    RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
                ) && active_graphics_protocol.is_some()
                    && (display_cells.0 < 72
                        || display_cells.1 < 20
                        || usize::from(display_cells.0)
                            .saturating_mul(usize::from(display_cells.1))
                            > HYBRID_GRAPHICS_MAX_CELLS)
                {
                    active_graphics_protocol = None;
                    last_osd_notice = Some(
                        "graphics fallback: text (resize/small terminal safeguard)".to_owned(),
                    );
                    osd_until = Some(Instant::now() + Duration::from_secs(3));
                }
                render_cells = resize_runtime_frame(
                    &mut terminal,
                    &mut frame,
                    &config,
                    display_cells,
                    active_graphics_protocol.is_some(),
                );
                if active_graphics_protocol.is_some() {
                    last_osd_notice = Some(format!(
                        "resize: display={}x{} render={}x{}",
                        display_cells.0, display_cells.1, render_cells.0, render_cells.1
                    ));
                    osd_until = Some(Instant::now() + Duration::from_secs(2));
                }
            }
        }
        if resize_recovery_pending {
            match terminal.size() {
                Ok((tw, th)) if !is_terminal_size_unstable(tw, th) => {
                    let (rw, rh, _) = cap_render_size(tw, th);
                    display_cells = (rw.max(1), rh.max(1));
                    if matches!(
                        config.output_mode,
                        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
                    ) && active_graphics_protocol.is_some()
                        && (display_cells.0 < 72
                            || display_cells.1 < 20
                            || usize::from(display_cells.0)
                                .saturating_mul(usize::from(display_cells.1))
                                > HYBRID_GRAPHICS_MAX_CELLS)
                    {
                        active_graphics_protocol = None;
                    }
                    render_cells = resize_runtime_frame(
                        &mut terminal,
                        &mut frame,
                        &config,
                        display_cells,
                        active_graphics_protocol.is_some(),
                    );
                    distance_clamp_guard.reset();
                    screen_fit.on_resize();
                    exposure_auto_boost.on_resize();
                    render_scratch.reset_exposure();
                    last_render_stats = RenderStats::default();
                    resize_recovery_pending = false;
                    last_osd_notice = Some(format!(
                        "resize recovered: display={}x{} render={}x{}",
                        display_cells.0, display_cells.1, render_cells.0, render_cells.1
                    ));
                    osd_until = Some(Instant::now() + Duration::from_secs(2));
                }
                _ => {
                    center_lock_state.reset();
                    last_osd_notice =
                        Some("resize unstable: waiting for terminal recovery".to_owned());
                    osd_until = Some(Instant::now() + Duration::from_secs(2));
                    thread::sleep(Duration::from_millis(16));
                    continue;
                }
            }
        }
        if input.status_changed {
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.stage_changed {
            last_osd_notice = Some(format!("stage={}", stage_level));
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.center_lock_blocked_pan {
            last_osd_notice = Some("center-lock on: pan disabled (press t to unlock)".to_owned());
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.center_lock_auto_disabled {
            last_osd_notice = Some("center-lock off: freefly active".to_owned());
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.zoom_changed {
            screen_fit.on_manual_zoom();
        }
        if input.freefly_toggled {
            let entered_freefly = runtime_camera.toggle_freefly(loaded_camera_track.is_some());
            if entered_freefly {
                center_lock_restore_after_freefly = center_lock_enabled;
                if center_lock_enabled {
                    center_lock_enabled = false;
                    center_lock_state.reset();
                }
                last_osd_notice = Some("freefly on (track paused)".to_owned());
            } else {
                if center_lock_restore_after_freefly && !center_lock_enabled {
                    center_lock_enabled = true;
                    center_lock_state.reset();
                }
                last_osd_notice = Some(if runtime_camera.track_enabled {
                    if center_lock_enabled {
                        "freefly off (track resumed, center-lock restored)".to_owned()
                    } else {
                        "freefly off (track resumed)".to_owned()
                    }
                } else {
                    if center_lock_enabled {
                        "freefly off (center-lock restored)".to_owned()
                    } else {
                        "freefly off".to_owned()
                    }
                });
            }
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
        if input.last_key == Some("c") {
            freefly_state = initial_freefly_state;
        }
        if matches!(config.mode, RenderMode::Ascii) && ascii_force_color_active {
            if input.last_key == Some("n") {
                last_osd_notice = Some("ascii color is forced: ansi".to_owned());
                osd_until = Some(Instant::now() + Duration::from_secs(2));
            }
            color_mode = ColorMode::Ansi;
        }
        if input.last_key == Some("n") {
            requested_color_mode =
                resolve_effective_color_mode(config.mode, color_mode, ascii_force_color_active);
            color_recovery.set_requested(requested_color_mode, requested_ansi_quantization);
            color_recovery.apply(
                &mut color_mode,
                &mut ansi_quantization,
                config.mode,
                ascii_force_color_active,
            );
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
        let interpolated_wall = sim_time + sim_accum / fixed_step * fixed_step;
        let animation_time = compute_animation_time(
            &mut continuous_sync_state,
            config.sync_policy,
            dt,
            interpolated_wall,
            elapsed_audio,
            sync_speed,
            sync_offset_ms,
            config.sync_hard_snap_ms,
            config.sync_kp,
            clip_duration,
        );
        pipeline.prepare_frame(&scene, animation_time, animation_index);
        let rotation = if animation_index.is_some() {
            0.0
        } else if model_spin_enabled {
            elapsed_wall * 0.9
        } else {
            0.0
        };
        if animation_index.is_none() && rotation.abs() > 0.0 {
            terminal.force_full_repaint();
        }
        let detected_cell_aspect = if config.cell_aspect_mode == CellAspectMode::Auto {
            detect_terminal_cell_aspect()
        } else {
            None
        };
        let target_aspect = resolve_cell_aspect(&config, detected_cell_aspect);
        effective_aspect_state += (target_aspect - effective_aspect_state) * 0.22;
        let effective_aspect = effective_aspect_state.clamp(0.30, 1.20);
        if active_graphics_protocol.is_some()
            && matches!(config.output_mode, RenderOutputMode::KittyHq)
        {
            let target_internal =
                kitty_internal_res_for_lod(kitty_internal_res_base, adaptive_quality.lod_level);
            if config.kitty_internal_res != target_internal {
                config.kitty_internal_res = target_internal;
                render_cells =
                    resize_runtime_frame(&mut terminal, &mut frame, &config, display_cells, true);
                last_osd_notice = Some(format!(
                    "kitty-hq adapt: internal={}x{} (lod={})",
                    render_cells.0, render_cells.1, adaptive_quality.lod_level
                ));
                osd_until = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        let mut frame_config = config.clone();
        frame_config.cell_aspect_mode = CellAspectMode::Manual;
        frame_config.cell_aspect = effective_aspect;
        frame_config.center_lock = center_lock_enabled;
        frame_config.center_lock_mode = center_lock_mode;
        frame_config.stage_level = stage_level.min(4);
        frame_config.color_mode =
            resolve_effective_color_mode(frame_config.mode, color_mode, ascii_force_color_active);
        frame_config.ansi_quantization = ansi_quantization;
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
        frame_config.exposure_bias = (exposure_bias + exposure_auto_boost.boost).clamp(-0.5, 0.8);

        apply_adaptive_quality_tuning(
            &mut frame_config,
            base_triangle_stride,
            base_min_triangle_area_px2,
            adaptive_quality.lod_level,
        );
        let prev_subject_height_ratio = if last_render_stats.subject_visible_height_ratio > 0.0 {
            last_render_stats.subject_visible_height_ratio
        } else {
            last_render_stats.visible_height_ratio
        };
        apply_distant_subject_clarity_boost(&mut frame_config, prev_subject_height_ratio);
        apply_face_focus_detail_boost(&mut frame_config, prev_subject_height_ratio);

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
        let effective_zoom = (user_zoom * screen_fit.auto_zoom_gain).clamp(0.20, 8.0);
        let zoom_repaint_threshold = if terminal_profile.is_ghostty {
            0.20
        } else {
            0.12
        };
        let repaint_due_to_zoom = (effective_zoom - 1.0).abs() > zoom_repaint_threshold
            || focus_offset.length_squared() > 0.01
            || camera_height_offset.abs() > 0.01;
        if repaint_due_to_zoom {
            if terminal_profile.is_ghostty {
                ghostty_zoom_repaint_due = Some(Instant::now() + Duration::from_millis(45));
            } else {
                terminal.force_full_repaint();
            }
        }
        if terminal_profile.is_ghostty
            && ghostty_zoom_repaint_due.is_some_and(|due| Instant::now() >= due)
        {
            terminal.force_full_repaint();
            ghostty_zoom_repaint_due = None;
        }
        let auto_radius_shrink = auto_radius_guard.shrink_ratio;
        if !center_lock_enabled || matches!(runtime_camera.control_mode, CameraControlMode::FreeFly)
        {
            center_lock_state.reset();
        }
        let mut camera = if matches!(runtime_camera.control_mode, CameraControlMode::FreeFly) {
            freefly_camera(freefly_state)
        } else {
            orbit_camera(
                orbit_state.angle + angle_jitter,
                (framing.radius * effective_zoom * radius_mul * (1.0 - auto_radius_shrink))
                    .clamp(0.2, 1000.0),
                (framing.camera_height + camera_height_offset + height_off).clamp(-1000.0, 1000.0),
                framing.focus
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
            )
        };
        if runtime_camera.track_enabled {
            if let Some(track) = loaded_camera_track.as_ref() {
                if let Some(vmd_pose) =
                    track
                        .sampler
                        .sample_pose(animation_time, track.transform, true)
                {
                    match runtime_camera.active_track_mode {
                        CameraMode::Off => {}
                        CameraMode::Vmd => {
                            camera.eye = vmd_pose.eye;
                            camera.target = vmd_pose.target;
                            camera.up = vmd_pose.up;
                            frame_config.fov_deg = vmd_pose.fov_deg;
                        }
                        CameraMode::Blend => {
                            camera.eye = camera.eye.lerp(vmd_pose.eye, 0.70);
                            camera.target = camera.target.lerp(vmd_pose.target, 0.70);
                            camera.up = camera.up.lerp(vmd_pose.up, 0.70).normalize_or_zero();
                            if camera.up.length_squared() <= f32::EPSILON {
                                camera.up = Vec3::Y;
                            }
                            frame_config.fov_deg =
                                frame_config.fov_deg * 0.30 + vmd_pose.fov_deg * 0.70;
                        }
                    }
                }
            }
        }
        let subject_target = if let Some(node_index) = scene.root_center_node {
            pipeline
                .globals()
                .get(node_index)
                .copied()
                .unwrap_or(glam::Mat4::IDENTITY)
                .transform_point3(Vec3::ZERO)
        } else {
            framing.focus
        };
        if center_lock_enabled && !matches!(runtime_camera.control_mode, CameraControlMode::FreeFly)
        {
            center_lock_state.apply_camera_space(
                &last_render_stats,
                center_lock_mode,
                frame.width,
                frame.height,
                &mut camera,
                frame_config.fov_deg,
                frame_config.cell_aspect,
                extent_y,
            );
        }
        let min_dist = distance_clamp_guard.apply(&mut camera, subject_target, extent_y, 0.35);
        let camera_dist = (camera.eye - subject_target).length().max(min_dist);
        let (dyn_near, dyn_far) =
            dynamic_clip_planes(min_dist, extent_y, camera_dist, has_stage_mesh);
        frame_config.near = dyn_near;
        frame_config.far = dyn_far;

        let stats = render_frame_with_backend(
            &mut gpu_renderer_state,
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
        let subject_height_ratio = if stats.subject_visible_height_ratio > 0.0 {
            stats.subject_visible_height_ratio
        } else {
            stats.visible_height_ratio
        };
        let subject_visible_ratio = if stats.subject_visible_ratio > 0.0 {
            stats.subject_visible_ratio
        } else {
            stats.visible_cell_ratio
        };
        if runtime_camera.track_enabled {
            if subject_visible_ratio < 0.0015 {
                track_lost_streak = track_lost_streak.saturating_add(1);
            } else {
                track_lost_streak = 0;
            }
            let centroid = stats.subject_centroid_px.or(stats.visible_centroid_px);
            if center_lock_enabled {
                if let Some((cx, cy)) = centroid {
                    let fw = f32::from(frame.width.max(1));
                    let fh = f32::from(frame.height.max(1));
                    let nx = ((cx / fw - 0.5) * 2.0).clamp(-2.0, 2.0);
                    let ny = ((cy / fh - 0.5) * 2.0).clamp(-2.0, 2.0);
                    if nx.abs() > 0.55 || ny.abs() > 0.55 {
                        center_drift_streak = center_drift_streak.saturating_add(1);
                    } else {
                        center_drift_streak = 0;
                    }
                } else {
                    center_drift_streak = center_drift_streak.saturating_add(1);
                }
            } else {
                center_drift_streak = 0;
            }
            if center_drift_streak >= 18 {
                runtime_camera.track_enabled = false;
                center_drift_streak = 0;
                track_lost_streak = 0;
                center_lock_state.reset();
                last_osd_notice = Some(
                    "camera track drifted off-center: fallback orbit (toggle f to retry)"
                        .to_owned(),
                );
                osd_until = Some(Instant::now() + Duration::from_secs(3));
            }
            if track_lost_streak >= 24 {
                runtime_camera.track_enabled = false;
                track_lost_streak = 0;
                center_drift_streak = 0;
                center_lock_state.reset();
                last_osd_notice = Some(
                    "camera track lost subject: fallback orbit (toggle f to retry)".to_owned(),
                );
                osd_until = Some(Instant::now() + Duration::from_secs(3));
            }
        } else {
            track_lost_streak = 0;
            center_drift_streak = 0;
        }
        auto_radius_guard.update(
            subject_height_ratio,
            center_lock_enabled && matches!(braille_profile, BrailleProfile::Safe),
        );
        screen_fit.update(subject_height_ratio, frame_config.mode, center_lock_enabled);
        exposure_auto_boost.update(subject_visible_ratio);

        if visibility_watchdog.observe(stats.visible_cell_ratio) {
            visibility_watchdog.reset();
            user_zoom = 1.0;
            focus_offset = Vec3::ZERO;
            camera_height_offset = 0.0;
            exposure_bias = (exposure_bias + 0.08).clamp(-0.5, 0.8);
            center_lock_state.reset();
            auto_radius_guard = AutoRadiusGuard::default();
            distance_clamp_guard.reset();
            screen_fit.on_resize();
            exposure_auto_boost.on_resize();
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
                sync_profile.as_ref().map(|profile| profile.hit),
                sync_profile_dirty,
                continuous_sync_state.drift_ema,
                continuous_sync_state.hard_snap_count,
                last_osd_notice.as_deref(),
            );
            overlay_osd(&mut frame, &status);
        }

        if active_graphics_protocol.is_some()
            && usize::from(display_cells.0).saturating_mul(usize::from(display_cells.1))
                > HYBRID_GRAPHICS_MAX_CELLS
        {
            active_graphics_protocol = None;
            resize_runtime_frame(&mut terminal, &mut frame, &config, display_cells, false);
            terminal.force_full_repaint();
            last_osd_notice = Some("graphics fallback: text (terminal too large)".to_owned());
            osd_until = Some(Instant::now() + Duration::from_secs(3));
        }

        let present_started = Instant::now();
        let present_result = if let Some(protocol) = active_graphics_protocol {
            terminal.present_graphics(
                &frame,
                protocol,
                frame_config.kitty_transport,
                frame_config.kitty_compression,
                frame_config.kitty_pipeline_mode,
                frame_config.recover_strategy,
                frame_config.kitty_scale,
                display_cells,
                input.resized || resize_recovery_pending,
            )
        } else if matches!(frame_config.color_mode, ColorMode::Ansi) {
            terminal.present(&frame, true, ansi_quantization)
        } else {
            terminal.present(&frame, false, ansi_quantization)
        };
        if let Err(err) = present_result {
            if active_graphics_protocol.is_some() {
                if matches!(
                    config.output_mode,
                    RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
                ) {
                    active_graphics_protocol = None;
                    resize_runtime_frame(&mut terminal, &mut frame, &config, display_cells, false);
                    terminal.force_full_repaint();
                    last_osd_notice = Some("graphics fallback: text".to_owned());
                    osd_until = Some(Instant::now() + Duration::from_secs(3));
                    continue;
                }
                return Err(err);
            }

            if is_retryable_io_error(&err) {
                io_failure_count = io_failure_count.saturating_add(1);
                if io_failure_count >= 3 {
                    io_failure_count = 0;
                    color_recovery.degrade(ascii_force_color_active, frame_config.mode);
                    color_recovery.apply(
                        &mut color_mode,
                        &mut ansi_quantization,
                        frame_config.mode,
                        ascii_force_color_active,
                    );
                    terminal.set_present_mode(PresentMode::FullFallback);
                    last_osd_notice = Some(format!(
                        "io fallback: {}",
                        color_path_label(color_mode, ansi_quantization)
                    ));
                    osd_until = Some(Instant::now() + Duration::from_secs(3));
                }
                continue;
            }
            io_failure_count = io_failure_count.saturating_add(1);
            if io_failure_count >= 3 {
                io_failure_count = 0;
                color_recovery.degrade(ascii_force_color_active, frame_config.mode);
                color_recovery.apply(
                    &mut color_mode,
                    &mut ansi_quantization,
                    frame_config.mode,
                    ascii_force_color_active,
                );
                terminal.set_present_mode(PresentMode::FullFallback);
                last_osd_notice = Some(format!(
                    "error fallback: {}",
                    color_path_label(color_mode, ansi_quantization)
                ));
                osd_until = Some(Instant::now() + Duration::from_secs(3));
                continue;
            }
            return Err(err);
        }
        if active_graphics_protocol.is_some() {
            let present_ms = present_started.elapsed().as_secs_f32() * 1000.0;
            if present_ms > HYBRID_GRAPHICS_SLOW_FRAME_MS {
                graphics_slow_streak = graphics_slow_streak.saturating_add(1);
            } else {
                graphics_slow_streak = graphics_slow_streak.saturating_sub(1);
            }
            if matches!(
                config.output_mode,
                RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
            ) && graphics_slow_streak >= HYBRID_GRAPHICS_SLOW_STREAK_LIMIT
            {
                active_graphics_protocol = None;
                graphics_slow_streak = 0;
                resize_runtime_frame(&mut terminal, &mut frame, &config, display_cells, false);
                terminal.force_full_repaint();
                last_osd_notice = Some(format!("graphics fallback: text ({present_ms:.1}ms)"));
                osd_until = Some(Instant::now() + Duration::from_secs(3));
                continue;
            }
        } else {
            graphics_slow_streak = 0;
        }
        io_failure_count = 0;
        if color_recovery.on_present_success() {
            color_recovery.apply(
                &mut color_mode,
                &mut ansi_quantization,
                frame_config.mode,
                ascii_force_color_active,
            );
            last_osd_notice = Some(format!(
                "color recover: {}",
                color_path_label(color_mode, ansi_quantization)
            ));
            osd_until = Some(Instant::now() + Duration::from_secs(2));
        }
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
    if let Some(profile) = sync_profile.as_ref() {
        if sync_profile_dirty
            && matches!(profile.mode, SyncProfileMode::Auto | SyncProfileMode::Write)
        {
            if let Err(err) = persist_sync_profile_offset(profile, sync_offset_ms) {
                eprintln!(
                    "warning: failed to save sync profile {}: {err}",
                    profile.store_path.display()
                );
            }
        }
    }
    cleanup_shm_registry();
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

    let focus_mask = focus_node_mask(scene);
    let (mut min, mut max, mut points) =
        collect_scene_points(scene, &globals, focus_mask.as_deref());
    if points.is_empty() {
        (min, max, points) = collect_scene_points(scene, &globals, None);
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

fn focus_node_mask(scene: &SceneCpu) -> Option<Vec<bool>> {
    let root = scene.root_center_node?;
    if root >= scene.nodes.len() {
        return None;
    }
    let mut mask = vec![false; scene.nodes.len()];
    let mut stack = vec![root];
    while let Some(node_index) = stack.pop() {
        if node_index >= scene.nodes.len() || mask[node_index] {
            continue;
        }
        mask[node_index] = true;
        stack.extend(scene.nodes[node_index].children.iter().copied());
    }
    Some(mask)
}

fn collect_scene_points(
    scene: &SceneCpu,
    globals: &[glam::Mat4],
    focus_mask: Option<&[bool]>,
) -> (Vec3, Vec3, Vec<Vec3>) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut points = Vec::new();
    for instance in &scene.mesh_instances {
        if focus_mask
            .and_then(|mask| mask.get(instance.node_index))
            .copied()
            == Some(false)
        {
            continue;
        }
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
    (min, max, points)
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

fn render_config_from_run(args: &RunArgs, visual: &ResolvedVisualOptions) -> RenderConfig {
    let mode: RenderMode = args.mode.into();
    let color_mode = resolve_effective_color_mode(
        mode,
        visual
            .color_mode
            .unwrap_or_else(|| default_color_mode_for_mode(mode)),
        visual.ascii_force_color,
    );
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        output_mode: visual.output_mode,
        graphics_protocol: visual.graphics_protocol,
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
        stage_role: if visual.stage_as_sub_only {
            StageRole::Sub
        } else {
            visual.stage_role
        },
        stage_luma_cap: visual.stage_luma_cap,
        recover_color_auto: visual.recover_color_auto,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        ascii_force_color: visual.ascii_force_color,
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
        texture_v_origin: visual.texture_v_origin,
        texture_sampler: visual.texture_sampler,
        clarity_profile: visual.clarity_profile,
        ansi_quantization: visual.ansi_quantization,
        model_lift: visual.model_lift,
        edge_accent_strength: visual.edge_accent_strength,
        bg_suppression: visual.bg_suppression,
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
        sync_policy: SyncPolicy::Continuous,
        sync_hard_snap_ms: 120,
        sync_kp: 0.15,
        contrast_floor: 0.10,
        contrast_gamma: 0.90,
        fog_scale: 1.0,
        triangle_stride: 1,
        min_triangle_area_px2: 0.0,
    }
}

fn render_config_from_start(args: &StartArgs, visual: &ResolvedVisualOptions) -> RenderConfig {
    let mode: RenderMode = args.mode.into();
    let color_mode = resolve_effective_color_mode(
        mode,
        visual
            .color_mode
            .unwrap_or_else(|| default_color_mode_for_mode(mode)),
        visual.ascii_force_color,
    );
    RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        output_mode: visual.output_mode,
        graphics_protocol: visual.graphics_protocol,
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
        stage_role: if visual.stage_as_sub_only {
            StageRole::Sub
        } else {
            visual.stage_role
        },
        stage_luma_cap: visual.stage_luma_cap,
        recover_color_auto: visual.recover_color_auto,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        ascii_force_color: visual.ascii_force_color,
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
        texture_v_origin: visual.texture_v_origin,
        texture_sampler: visual.texture_sampler,
        clarity_profile: visual.clarity_profile,
        ansi_quantization: visual.ansi_quantization,
        model_lift: visual.model_lift,
        edge_accent_strength: visual.edge_accent_strength,
        bg_suppression: visual.bg_suppression,
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
        sync_policy: SyncPolicy::Continuous,
        sync_hard_snap_ms: 120,
        sync_kp: 0.15,
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

fn discover_camera_vmds(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() || !dir.is_dir() {
        return Vec::new();
    }
    let mut files = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("vmd"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[cfg(test)]
fn discover_default_camera_vmd(dir: &Path) -> Option<PathBuf> {
    resolve_camera_vmd_selector(&discover_camera_vmds(dir), "auto")
}

fn resolve_camera_vmd_selector(files: &[PathBuf], selector: &str) -> Option<PathBuf> {
    let raw = selector.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("none") {
        return None;
    }
    if raw.eq_ignore_ascii_case("auto") {
        if let Some(preferred) = files.iter().find(|path| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(|name| name.to_ascii_lowercase().contains("world_is_mine"))
                .unwrap_or(false)
        }) {
            return Some(preferred.clone());
        }
        return files.first().cloned();
    }

    let as_path = PathBuf::from(raw);
    if as_path.exists() && as_path.is_file() {
        return Some(as_path);
    }
    let needle = raw.to_ascii_lowercase();
    files
        .iter()
        .find(|path| {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            file_name == needle || stem == needle || file_name.contains(&needle)
        })
        .cloned()
}

fn resolved_stage_dir(cli_stage_dir: &Path, runtime_cfg: &GasciiConfig) -> PathBuf {
    let cli_default = Path::new("assets/stage");
    if cli_stage_dir == cli_default && runtime_cfg.stage_dir != cli_default {
        runtime_cfg.stage_dir.clone()
    } else {
        cli_stage_dir.to_path_buf()
    }
}

fn resolved_camera_dir(cli_camera_dir: &Path, runtime_cfg: &GasciiConfig) -> PathBuf {
    let cli_default = Path::new("assets/camera");
    if cli_camera_dir == cli_default && runtime_cfg.camera_dir != cli_default {
        runtime_cfg.camera_dir.clone()
    } else {
        cli_camera_dir.to_path_buf()
    }
}

fn discover_stage_sets(root: &Path) -> Vec<StageChoice> {
    if !root.exists() || !root.is_dir() {
        return Vec::new();
    }
    let mut dirs = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();

    let mut stages = Vec::new();
    for dir in dirs {
        let mut renderable_files = Vec::new();
        let mut pmx_files = Vec::new();
        discover_stage_files_recursive(&dir, &mut renderable_files, &mut pmx_files);
        renderable_files.sort();
        pmx_files.sort();

        let status = if !renderable_files.is_empty() {
            StageStatus::Ready
        } else if !pmx_files.is_empty() {
            StageStatus::NeedsConvert
        } else {
            StageStatus::Invalid
        };
        let transform = load_stage_transform(&dir.join("stage.meta.toml"));
        stages.push(StageChoice {
            name: dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<invalid>")
                .to_owned(),
            status,
            render_path: renderable_files.first().cloned(),
            pmx_path: pmx_files.first().cloned(),
            transform,
        });
    }
    stages
}

fn resolved_stage_selector(cli_stage: Option<&str>, runtime_cfg: &GasciiConfig) -> String {
    cli_stage
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| runtime_cfg.stage_selection.clone())
}

fn resolve_stage_choice_from_selector(
    entries: &[StageChoice],
    selector: &str,
) -> Option<StageChoice> {
    let trimmed = selector.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return entries
            .iter()
            .find(|entry| matches!(entry.status, StageStatus::Ready))
            .cloned();
    }
    if trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed == "없음"
    {
        return None;
    }

    let selector_path = Path::new(trimmed);
    if selector_path.exists() {
        if selector_path.is_dir() {
            if let Some(dir_name) = selector_path.file_name().and_then(|n| n.to_str()) {
                if let Some(found) = entries
                    .iter()
                    .find(|entry| entry.name.eq_ignore_ascii_case(dir_name))
                {
                    return Some(found.clone());
                }
            }
        }
        let selector_abs = selector_path
            .canonicalize()
            .unwrap_or_else(|_| selector_path.to_path_buf());
        if let Some(found) = entries.iter().find(|entry| {
            entry
                .render_path
                .as_ref()
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|p| p == selector_abs)
        }) {
            return Some(found.clone());
        }
    }

    entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(trimmed))
        .cloned()
}

fn discover_stage_files_recursive(
    root: &Path,
    renderable_files: &mut Vec<PathBuf>,
    pmx_files: &mut Vec<PathBuf>,
) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());
            match ext.as_deref() {
                Some("glb" | "gltf" | "obj") => renderable_files.push(path),
                Some("pmx") => pmx_files.push(path),
                _ => {}
            }
        }
    }
}

fn load_stage_transform(path: &Path) -> StageTransform {
    let Ok(content) = fs::read_to_string(path) else {
        return StageTransform::default();
    };
    let mut transform = StageTransform::default();
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .replace(' ', "_");
        let value = raw_value.trim();
        match key.as_str() {
            "offset" => {
                if let Some(v) = parse_meta_vec3(value) {
                    transform.offset = v;
                }
            }
            "rot" | "rotation" | "rotation_deg" => {
                if let Some(v) = parse_meta_vec3(value) {
                    transform.rotation_deg = v;
                }
            }
            "scale" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    transform.scale = parsed.clamp(0.01, 100.0);
                }
            }
            _ => {}
        }
    }
    transform
}

fn parse_meta_vec3(value: &str) -> Option<[f32; 3]> {
    let trimmed = value.trim();
    let body = trimmed
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(trimmed);
    let parts = body
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok())
        .collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    Some([parts[0]?, parts[1]?, parts[2]?])
}

fn apply_stage_transform(scene: &mut SceneCpu, transform: StageTransform) {
    if scene.nodes.is_empty() {
        return;
    }
    let rotation = Quat::from_euler(
        glam::EulerRot::XYZ,
        transform.rotation_deg[0].to_radians(),
        transform.rotation_deg[1].to_radians(),
        transform.rotation_deg[2].to_radians(),
    );
    let root_index = scene.nodes.len();
    let mut children = Vec::new();
    for (index, node) in scene.nodes.iter_mut().enumerate() {
        if node.parent.is_none() {
            node.parent = Some(root_index);
            children.push(index);
        }
    }
    scene.nodes.push(Node {
        name: Some("StageTransformRoot".to_owned()),
        parent: None,
        children,
        base_translation: Vec3::new(
            transform.offset[0],
            transform.offset[1],
            transform.offset[2],
        ),
        base_rotation: rotation,
        base_scale: Vec3::splat(transform.scale),
    });
}

fn load_scene_file(path: &Path) -> Result<SceneCpu> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "glb" | "gltf" => loader::load_gltf(path),
        "obj" => loader::load_obj(path),
        "pmx" => loader::load_pmx(path),
        other => bail!(
            "unsupported scene file extension for runtime merge: {} ({other})",
            path.display()
        ),
    }
}

fn merge_scenes(mut base: SceneCpu, mut overlay: SceneCpu) -> SceneCpu {
    let texture_offset = base.textures.len();
    base.textures.append(&mut overlay.textures);

    let material_offset = base.materials.len();
    for material in &mut overlay.materials {
        material.base_color_texture = material.base_color_texture.map(|idx| idx + texture_offset);
    }
    base.materials.append(&mut overlay.materials);

    let mesh_offset = base.meshes.len();
    for mesh in &mut overlay.meshes {
        mesh.material_index = mesh.material_index.map(|idx| idx + material_offset);
    }
    base.meshes.append(&mut overlay.meshes);

    let node_offset = base.nodes.len();
    for node in &mut overlay.nodes {
        node.parent = node.parent.map(|idx| idx + node_offset);
        for child in &mut node.children {
            *child += node_offset;
        }
    }
    let overlay_root = overlay.root_center_node.map(|idx| idx + node_offset);
    base.nodes.append(&mut overlay.nodes);

    let skin_offset = base.skins.len();
    for skin in &mut overlay.skins {
        for joint in &mut skin.joints {
            *joint += node_offset;
        }
    }
    base.skins.append(&mut overlay.skins);

    for instance in &mut overlay.mesh_instances {
        instance.mesh_index += mesh_offset;
        instance.node_index += node_offset;
        instance.skin_index = instance.skin_index.map(|idx| idx + skin_offset);
        instance.layer = MeshLayer::Stage;
    }
    base.mesh_instances.append(&mut overlay.mesh_instances);

    for clip in &mut overlay.animations {
        for channel in &mut clip.channels {
            channel.node_index += node_offset;
        }
    }
    base.animations.append(&mut overlay.animations);

    if base.root_center_node.is_none() {
        base.root_center_node = overlay_root;
    }
    base
}

fn validated_terminal_size(terminal: &TerminalSession) -> Result<(u16, u16)> {
    let (w, h) = terminal.size()?;
    if !is_terminal_size_unstable(w, h) {
        return Ok((w, h));
    }
    let env_w = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0 && *v < u16::MAX);
    let env_h = std::env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0 && *v < u16::MAX);
    match (env_w, env_h) {
        (Some(width), Some(height)) if !is_terminal_size_unstable(width, height) => {
            Ok((width, height))
        }
        _ => bail!(
            "terminal size unavailable (got {w}x{h}). set COLUMNS/LINES or use a real TTY terminal"
        ),
    }
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

fn run_ghostty_font_shortcut(key: &str) {
    if !TerminalProfile::detect().is_ghostty {
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
    (clip / audio).clamp(0.25, 4.0)
}

#[allow(clippy::too_many_arguments)]
fn compute_animation_time(
    state: &mut ContinuousSyncState,
    policy: SyncPolicy,
    dt_wall: f32,
    elapsed_wall: f32,
    elapsed_audio: Option<f32>,
    speed_factor: f32,
    sync_offset_ms: i32,
    hard_snap_ms: u32,
    sync_kp: f32,
    clip_duration: Option<f32>,
) -> f32 {
    let offset = (sync_offset_ms as f32) / 1000.0;
    let hard_snap_sec = (hard_snap_ms as f32 / 1000.0).clamp(0.005, 5.0);
    let kp = sync_kp.clamp(0.01, 1.0);
    let dt = dt_wall.max(0.0);

    let target_audio = elapsed_audio.map(|seconds| seconds * speed_factor + offset);

    match policy {
        SyncPolicy::Manual => {
            if !state.initialized {
                state.anim_time = elapsed_wall + offset;
                state.initialized = true;
            } else {
                state.anim_time += dt;
            }
            state.drift_ema *= 0.92;
        }
        SyncPolicy::Fixed => {
            state.anim_time = target_audio.unwrap_or(elapsed_wall + offset);
            state.initialized = true;
            state.drift_ema *= 0.92;
        }
        SyncPolicy::Continuous => {
            if let Some(target) = target_audio {
                if !state.initialized {
                    state.anim_time = target;
                    state.initialized = true;
                    state.drift_ema = 0.0;
                } else {
                    let err = target - state.anim_time;
                    state.drift_ema += (err.abs() - state.drift_ema) * 0.08;
                    if err.abs() > hard_snap_sec {
                        state.anim_time = target;
                        state.hard_snap_count = state.hard_snap_count.saturating_add(1);
                    } else {
                        let drift_gain = (state.drift_ema / hard_snap_sec).clamp(0.0, 1.0);
                        let long_drift_term = (err * 0.18 * drift_gain).clamp(-0.16, 0.16);
                        let rate = (speed_factor + kp * err + long_drift_term).clamp(0.25, 4.0);
                        state.anim_time += dt * rate;
                    }
                }
            } else {
                state.anim_time = elapsed_wall + offset;
                state.initialized = true;
                state.drift_ema *= 0.92;
            }
        }
    }

    if let Some(duration) = clip_duration.filter(|value| *value > f32::EPSILON) {
        state.anim_time = state.anim_time.rem_euclid(duration);
    }
    state.anim_time
}

fn load_camera_track(settings: &RuntimeCameraSettings) -> Option<LoadedCameraTrack> {
    if matches!(settings.mode, CameraMode::Off) {
        return None;
    }
    let path = settings.vmd_path.as_deref()?;
    let track = parse_vmd_camera(path).ok()?;
    let sampler = CameraTrackSampler::from_vmd(&track, settings.vmd_fps)?;
    let transform = MmdCameraTransform::from_preset(settings.align_preset, settings.unit_scale);
    Some(LoadedCameraTrack { sampler, transform })
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
    sync_profile_hit: Option<bool>,
    sync_profile_dirty: bool,
    drift_ema: f32,
    hard_snap_count: u32,
    notice: Option<&str>,
) -> String {
    let profile_label = match sync_profile_hit {
        Some(true) => "hit",
        Some(false) => "miss",
        None => "off",
    };
    let core = format!(
        "offset={sync_offset_ms}ms  speed={sync_speed:.4}x  aspect={effective_aspect:.3}  contrast={}  braille={:?}  color={:?}  camera={:?}  gain={reactive_gain:.2}  exp={exposure_bias:+.2}  stage={}  center={}  lod={}  target={target_ms:.1}ms  ema={frame_ema_ms:.1}ms  profile={}{}  drift={drift_ema:.4}  snaps={hard_snap_count}",
        contrast.label(),
        braille_profile,
        color_mode,
        cinematic_mode,
        stage_level,
        if center_lock { "on" } else { "off" },
        lod_level,
        profile_label,
        if sync_profile_dirty { "*" } else { "" },
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
    control_mode: &mut CameraControlMode,
    camera_look_speed: f32,
    freefly_state: &mut FreeFlyState,
) -> Result<RuntimeInputResult> {
    let mut result = RuntimeInputResult::default();
    while event::poll(Duration::from_millis(0))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('Q') => {
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
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Forward);
                        result.status_changed = true;
                        result.last_key = Some("w");
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Backward);
                        result.status_changed = true;
                        result.last_key = Some("s");
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Left);
                        result.status_changed = true;
                        result.last_key = Some("a");
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Right);
                        result.status_changed = true;
                        result.last_key = Some("d");
                    }
                }
                KeyCode::Char('q') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Down);
                        result.status_changed = true;
                        result.last_key = Some("q");
                    } else {
                        result.quit = true;
                        result.last_key = Some("q");
                        return Ok(result);
                    }
                }
                KeyCode::Char('e') => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_translate(freefly_state, FreeFlyDirection::Up);
                        result.status_changed = true;
                        result.last_key = Some("e");
                    } else {
                        *exposure_bias = (*exposure_bias - 0.04).clamp(-0.5, 0.8);
                        result.status_changed = true;
                        result.last_key = Some("e");
                    }
                }
                KeyCode::Char('E') => {
                    *exposure_bias = (*exposure_bias + 0.04).clamp(-0.5, 0.8);
                    result.status_changed = true;
                    result.last_key = Some("E");
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
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    result.freefly_toggled = true;
                    result.status_changed = true;
                    result.last_key = Some("f");
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    *center_lock_enabled = !*center_lock_enabled;
                    result.status_changed = true;
                    result.last_key = Some("t");
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
                KeyCode::Char('[') => {
                    *zoom = (*zoom + 0.08).clamp(0.2, 8.0);
                    result.zoom_changed = true;
                }
                KeyCode::Char(']') => {
                    *zoom = (*zoom - 0.08).clamp(0.2, 8.0);
                    result.zoom_changed = true;
                }
                KeyCode::Left => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_rotate(freefly_state, -0.06 * camera_look_speed, 0.0);
                        result.status_changed = true;
                        result.last_key = Some("left");
                    } else if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x -= 0.08;
                    }
                }
                KeyCode::Right => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_rotate(freefly_state, 0.06 * camera_look_speed, 0.0);
                        result.status_changed = true;
                        result.last_key = Some("right");
                    } else if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x += 0.08;
                    }
                }
                KeyCode::Up => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_rotate(freefly_state, 0.0, 0.05 * camera_look_speed);
                        result.status_changed = true;
                        result.last_key = Some("up");
                    } else if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.y += 0.08;
                        *camera_height_offset += 0.08;
                    }
                }
                KeyCode::Down => {
                    if matches!(*control_mode, CameraControlMode::FreeFly) {
                        if *center_lock_enabled {
                            *center_lock_enabled = false;
                            result.center_lock_auto_disabled = true;
                        }
                        freefly_rotate(freefly_state, 0.0, -0.05 * camera_look_speed);
                        result.status_changed = true;
                        result.last_key = Some("down");
                    } else if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.y -= 0.08;
                        *camera_height_offset -= 0.08;
                    }
                }
                KeyCode::Char('j') | KeyCode::Char('J') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x -= 0.08;
                    }
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.x += 0.08;
                    }
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    if *center_lock_enabled {
                        result.center_lock_blocked_pan = true;
                    } else {
                        focus_offset.y += 0.08;
                        *camera_height_offset += 0.08;
                    }
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
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
                    result.zoom_changed = true;
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
                if is_terminal_size_unstable(width, height) {
                    result.terminal_size_unstable = true;
                    result.resized_terminal = None;
                } else {
                    result.terminal_size_unstable = false;
                    result.resized_terminal = Some((width, height));
                }
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
    let visual = resolve_visual_options_for_bench(&args, &runtime_cfg);
    let mode: RenderMode = args.mode.into();
    let color_mode = resolve_effective_color_mode(
        mode,
        visual
            .color_mode
            .unwrap_or_else(|| default_color_mode_for_mode(mode)),
        visual.ascii_force_color,
    );
    let mut config = RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode,
        output_mode: visual.output_mode,
        graphics_protocol: visual.graphics_protocol,
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
        stage_role: if visual.stage_as_sub_only {
            StageRole::Sub
        } else {
            visual.stage_role
        },
        stage_luma_cap: visual.stage_luma_cap,
        recover_color_auto: visual.recover_color_auto,
        perf_profile: visual.perf_profile,
        detail_profile: visual.detail_profile,
        backend: visual.backend,
        color_mode,
        ascii_force_color: visual.ascii_force_color,
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
        texture_v_origin: visual.texture_v_origin,
        texture_sampler: visual.texture_sampler,
        clarity_profile: visual.clarity_profile,
        ansi_quantization: visual.ansi_quantization,
        model_lift: visual.model_lift,
        edge_accent_strength: visual.edge_accent_strength,
        bg_suppression: visual.bg_suppression,
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
        sync_policy: runtime_cfg.sync_policy,
        sync_hard_snap_ms: runtime_cfg.sync_hard_snap_ms,
        sync_kp: runtime_cfg.sync_kp,
        contrast_floor: 0.10,
        contrast_gamma: 0.90,
        fog_scale: 1.0,
        triangle_stride: 1,
        min_triangle_area_px2: 0.0,
    };
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
    let raw = gltf::Gltf::open(&args.glb)
        .with_context(|| format!("failed to parse glTF metadata: {}", args.glb.display()))?;
    let unsupported_required_extensions = loader::unsupported_required_extensions(&raw);
    let unsupported_used_extensions = loader::unsupported_used_extensions(&raw);
    let scene = loader::load_gltf(&args.glb)?;
    let extensions_required = raw
        .extensions_required()
        .map(|name| name.to_owned())
        .collect::<Vec<_>>();
    let extensions_used = raw
        .extensions_used()
        .map(|name| name.to_owned())
        .collect::<Vec<_>>();
    let mut khr_texture_transform_primitives = 0usize;
    let mut texcoord_override_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut texcoord_base_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut non_triangle_primitives = 0usize;
    let mut normal_texture_primitives = 0usize;
    let mut emissive_texture_primitives = 0usize;
    let mut occlusion_texture_primitives = 0usize;
    let mut metallic_roughness_texture_primitives = 0usize;
    let mut double_sided_materials = 0usize;
    for mesh in raw.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                non_triangle_primitives = non_triangle_primitives.saturating_add(1);
            }
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            if let Some(base_color_info) = pbr.base_color_texture() {
                let base_coord = base_color_info.tex_coord();
                *texcoord_base_counts.entry(base_coord).or_insert(0) += 1;
                if let Some(transform) = base_color_info.texture_transform() {
                    khr_texture_transform_primitives += 1;
                    if let Some(override_coord) = transform.tex_coord() {
                        *texcoord_override_counts.entry(override_coord).or_insert(0) += 1;
                    }
                }
            }
            if material.normal_texture().is_some() {
                normal_texture_primitives = normal_texture_primitives.saturating_add(1);
            }
            if material.emissive_texture().is_some() {
                emissive_texture_primitives = emissive_texture_primitives.saturating_add(1);
            }
            if material.occlusion_texture().is_some() {
                occlusion_texture_primitives = occlusion_texture_primitives.saturating_add(1);
            }
            if pbr.metallic_roughness_texture().is_some() {
                metallic_roughness_texture_primitives =
                    metallic_roughness_texture_primitives.saturating_add(1);
            }
            if material.double_sided() {
                double_sided_materials = double_sided_materials.saturating_add(1);
            }
        }
    }

    println!("file: {}", args.glb.display());
    println!(
        "extensions_required: {}",
        if extensions_required.is_empty() {
            "[]".to_owned()
        } else {
            format!("{extensions_required:?}")
        }
    );
    println!(
        "extensions_used: {}",
        if extensions_used.is_empty() {
            "[]".to_owned()
        } else {
            format!("{extensions_used:?}")
        }
    );
    println!(
        "unsupported_required_extensions: {}",
        if unsupported_required_extensions.is_empty() {
            "[]".to_owned()
        } else {
            format!("{unsupported_required_extensions:?}")
        }
    );
    println!(
        "unsupported_used_extensions: {}",
        if unsupported_used_extensions.is_empty() {
            "[]".to_owned()
        } else {
            format!("{unsupported_used_extensions:?}")
        }
    );
    println!(
        "khr_texture_transform_primitives: {}",
        khr_texture_transform_primitives
    );
    println!(
        "base_color_texcoord_distribution: {}",
        if texcoord_base_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texcoord_base_counts:?}")
        }
    );
    println!(
        "texcoord_override_distribution: {}",
        if texcoord_override_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texcoord_override_counts:?}")
        }
    );
    println!("non_triangle_primitives: {}", non_triangle_primitives);
    println!("normal_texture_primitives: {}", normal_texture_primitives);
    println!(
        "emissive_texture_primitives: {}",
        emissive_texture_primitives
    );
    println!(
        "occlusion_texture_primitives: {}",
        occlusion_texture_primitives
    );
    println!(
        "metallic_roughness_texture_primitives: {}",
        metallic_roughness_texture_primitives
    );
    println!("double_sided_materials: {}", double_sided_materials);
    println!("meshes: {}", scene.meshes.len());
    println!("mesh_instances: {}", scene.mesh_instances.len());
    println!("nodes: {}", scene.nodes.len());
    if let Some(root_idx) = scene.root_center_node {
        let root_name = scene
            .nodes
            .get(root_idx)
            .and_then(|node| node.name.as_deref())
            .unwrap_or("<unnamed>");
        println!("root_center_node: {} ({})", root_idx, root_name);
    } else {
        println!("root_center_node: none");
    }
    println!("skins: {}", scene.skins.len());
    println!("materials: {}", scene.materials.len());
    println!("textures: {}", scene.textures.len());
    let fallback_white_textures = scene
        .textures
        .iter()
        .filter(|texture| texture.source_format == "FallbackWhite")
        .count();
    println!("fallback_white_textures: {}", fallback_white_textures);
    println!(
        "renderer_material_coverage: baseColor/alpha/vertexColor/textureTransform only; normal/emissive/occlusion/PBR lighting are ignored by the terminal renderer"
    );
    println!("animations: {}", scene.animations.len());
    let mut texture_format_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut texture_color_space_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for texture in &scene.textures {
        *texture_format_counts
            .entry(texture.source_format.clone())
            .or_insert(0) += 1;
        let key = match texture.color_space {
            crate::scene::TextureColorSpace::Srgb => "sRGB",
            crate::scene::TextureColorSpace::Linear => "Linear",
        };
        *texture_color_space_counts.entry(key).or_insert(0) += 1;
    }
    let mut base_color_sampler_counts: BTreeMap<String, usize> = BTreeMap::new();
    for material in &scene.materials {
        let key = format!(
            "wrap=({:?},{:?}) filter=({:?},{:?})",
            material.base_color_wrap_s,
            material.base_color_wrap_t,
            material.base_color_min_filter,
            material.base_color_mag_filter
        );
        *base_color_sampler_counts.entry(key).or_insert(0) += 1;
    }
    println!(
        "texture_formats: {}",
        if texture_format_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texture_format_counts:?}")
        }
    );
    println!(
        "texture_color_spaces: {}",
        if texture_color_space_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{texture_color_space_counts:?}")
        }
    );
    println!(
        "base_color_sampler_distribution: {}",
        if base_color_sampler_counts.is_empty() {
            "{}".to_owned()
        } else {
            format!("{base_color_sampler_counts:?}")
        }
    );
    for (index, texture) in scene.textures.iter().enumerate() {
        let color_space = match texture.color_space {
            crate::scene::TextureColorSpace::Srgb => "sRGB",
            crate::scene::TextureColorSpace::Linear => "Linear",
        };
        println!(
            "texture[{index}]: {}x{} format={} color_space={} mips={}",
            texture.width,
            texture.height,
            texture.source_format,
            color_space,
            texture.mip_levels.len()
        );
    }
    for (index, material) in scene.materials.iter().enumerate() {
        println!(
            "material[{index}]: base_tex={:?} texcoord={} wrap=({:?},{:?}) filter=({:?},{:?}) alpha={:?} cutoff={:.3} double_sided={}",
            material.base_color_texture,
            material.base_color_tex_coord,
            material.base_color_wrap_s,
            material.base_color_wrap_t,
            material.base_color_min_filter,
            material.base_color_mag_filter,
            material.alpha_mode,
            material.alpha_cutoff,
            material.double_sided
        );
    }
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

#[derive(Debug, Clone, Copy)]
enum FreeFlyDirection {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

fn freefly_state_from_camera(camera: Camera, move_speed: f32) -> FreeFlyState {
    let forward = (camera.target - camera.eye).normalize_or_zero();
    let direction = if forward.length_squared() <= f32::EPSILON {
        Vec3::new(0.0, 0.0, -1.0)
    } else {
        forward
    };
    let pitch = direction.y.clamp(-1.0, 1.0).asin();
    let yaw = direction.z.atan2(direction.x);
    FreeFlyState {
        eye: camera.eye,
        target: camera.target,
        yaw,
        pitch,
        move_speed: move_speed.clamp(0.1, 8.0),
    }
}

fn freefly_forward(state: &FreeFlyState) -> Vec3 {
    let cp = state.pitch.cos();
    Vec3::new(
        state.yaw.cos() * cp,
        state.pitch.sin(),
        state.yaw.sin() * cp,
    )
    .normalize_or_zero()
}

fn freefly_camera(state: FreeFlyState) -> Camera {
    Camera {
        eye: state.eye,
        target: state.target,
        up: Vec3::Y,
    }
}

fn freefly_translate(state: &mut FreeFlyState, direction: FreeFlyDirection) {
    let mut forward = (state.target - state.eye).normalize_or_zero();
    if forward.length_squared() <= f32::EPSILON {
        forward = freefly_forward(state);
    }
    if forward.length_squared() <= f32::EPSILON {
        forward = Vec3::new(0.0, 0.0, -1.0);
    }
    let mut right = forward.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() <= f32::EPSILON {
        right = Vec3::X;
    }
    let up = Vec3::Y;
    let axis = match direction {
        FreeFlyDirection::Forward => forward,
        FreeFlyDirection::Backward => -forward,
        FreeFlyDirection::Left => -right,
        FreeFlyDirection::Right => right,
        FreeFlyDirection::Up => up,
        FreeFlyDirection::Down => -up,
    };
    let step = 0.12 * state.move_speed.clamp(0.1, 8.0);
    let delta = axis * step;
    state.eye += delta;
    state.target += delta;
}

fn freefly_rotate(state: &mut FreeFlyState, yaw_delta: f32, pitch_delta: f32) {
    state.yaw += yaw_delta;
    state.pitch = (state.pitch + pitch_delta).clamp(-1.45, 1.45);
    let forward = freefly_forward(state);
    if forward.length_squared() <= f32::EPSILON {
        return;
    }
    let distance = (state.target - state.eye).length().max(0.5);
    state.target = state.eye + forward * distance;
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
        use glam::{Quat, Vec3};

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
        };

        let index = resolve_animation_index(&scene, None).expect("animation index");
        assert_eq!(index, Some(1));
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
        assert!(
            stages
                .iter()
                .any(|s| s.name == "empty_stage" && matches!(s.status, StageStatus::Invalid))
        );
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
        let picked = discover_default_camera_vmd(&camera_dir).expect("picked");
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
