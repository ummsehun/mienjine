use std::path::PathBuf;

use crate::scene::{
    AnsiQuantization, BrailleProfile, CameraAlignPreset, CameraControlMode, CameraMode,
    CinematicCameraMode, ColorMode, ContrastProfile, KittyCompression, KittyTransport,
    RenderBackend, RenderConfig, RenderMode, RenderOutputMode,
};

mod pmx_physics;
mod quality;
mod quality_tuning;

pub(crate) use pmx_physics::PmxPhysicsState;
pub(crate) use quality::{
    apply_runtime_contrast_preset, dynamic_clip_planes, AutoRadiusGuard, CenterLockState,
    DistanceClampGuard, ExposureAutoBoost, RuntimeAdaptiveQuality, ScreenFitController,
    VisibilityWatchdog,
};
pub(crate) use quality_tuning::{
    apply_adaptive_quality_tuning, apply_distant_subject_clarity_boost,
    apply_face_focus_detail_boost, apply_pmx_surface_guardrails, jitter_scale_for_lod,
};

pub(crate) const SYNC_OFFSET_STEP_MS: i32 = 10;
pub(crate) const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;
pub(crate) const MAX_RENDER_COLS: u16 = 4096;
pub(crate) const MAX_RENDER_ROWS: u16 = 2048;
pub(crate) const VISIBILITY_LOW_THRESHOLD: f32 = 0.002;
pub(crate) const VISIBILITY_LOW_FRAMES_TO_RECOVER: u32 = 12;
pub(crate) const LOW_VIS_EXPOSURE_THRESHOLD: f32 = 0.008;
pub(crate) const LOW_VIS_EXPOSURE_TRIGGER_FRAMES: u32 = 6;
pub(crate) const LOW_VIS_EXPOSURE_RECOVER_THRESHOLD: f32 = 0.020;
pub(crate) const LOW_VIS_EXPOSURE_RECOVER_FRAMES: u32 = 24;
pub(crate) const MIN_VISIBLE_HEIGHT_RATIO: f32 = 0.10;
pub(crate) const MIN_VISIBLE_HEIGHT_TRIGGER_FRAMES: u32 = 10;
pub(crate) const MIN_VISIBLE_HEIGHT_RECOVER_RATIO: f32 = 0.16;
pub(crate) const MIN_VISIBLE_HEIGHT_RECOVER_FRAMES: u32 = 30;
pub(crate) const HYBRID_GRAPHICS_MAX_CELLS: usize = 24_000;
pub(crate) const HYBRID_GRAPHICS_SLOW_FRAME_MS: f32 = 45.0;
pub(crate) const HYBRID_GRAPHICS_SLOW_STREAK_LIMIT: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeContrastPreset {
    AdaptiveLow,
    AdaptiveNormal,
    AdaptiveHigh,
    Fixed,
}

impl RuntimeContrastPreset {
    pub(crate) fn from_profile(profile: ContrastProfile) -> Self {
        match profile {
            ContrastProfile::Adaptive => RuntimeContrastPreset::AdaptiveNormal,
            ContrastProfile::Fixed => RuntimeContrastPreset::Fixed,
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            RuntimeContrastPreset::AdaptiveLow => RuntimeContrastPreset::AdaptiveNormal,
            RuntimeContrastPreset::AdaptiveNormal => RuntimeContrastPreset::AdaptiveHigh,
            RuntimeContrastPreset::AdaptiveHigh => RuntimeContrastPreset::Fixed,
            RuntimeContrastPreset::Fixed => RuntimeContrastPreset::AdaptiveLow,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            RuntimeContrastPreset::AdaptiveLow => "adaptive-low",
            RuntimeContrastPreset::AdaptiveNormal => "adaptive-normal",
            RuntimeContrastPreset::AdaptiveHigh => "adaptive-high",
            RuntimeContrastPreset::Fixed => "fixed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ContinuousSyncState {
    pub(crate) anim_time: f32,
    pub(crate) initialized: bool,
    pub(crate) drift_ema: f32,
    pub(crate) hard_snap_count: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeCameraSettings {
    pub(crate) mode: CameraMode,
    pub(crate) align_preset: CameraAlignPreset,
    pub(crate) unit_scale: f32,
    pub(crate) vmd_fps: f32,
    pub(crate) vmd_path: Option<PathBuf>,
    pub(crate) look_speed: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ReactiveState {
    pub(crate) energy: f32,
    pub(crate) smoothed_energy: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameraShot {
    FullBody,
    UpperBody,
    FaceCloseup,
    Hands,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CameraDirectorState {
    pub(crate) shot: CameraShot,
    pub(crate) next_cut_at: f32,
    pub(crate) transition_started_at: f32,
    pub(crate) previous_radius_mul: f32,
    pub(crate) previous_height_offset: f32,
    pub(crate) previous_focus_y_offset: f32,
    pub(crate) radius_mul: f32,
    pub(crate) height_offset: f32,
    pub(crate) focus_y_offset: f32,
    pub(crate) face_time_accum: f32,
    pub(crate) total_time_accum: f32,
    pub(crate) jitter_phase: f32,
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
pub(crate) struct RuntimeInputResult {
    pub(crate) quit: bool,
    pub(crate) status_changed: bool,
    pub(crate) resized: bool,
    pub(crate) terminal_size_unstable: bool,
    pub(crate) resized_terminal: Option<(u16, u16)>,
    pub(crate) stage_changed: bool,
    pub(crate) center_lock_blocked_pan: bool,
    pub(crate) center_lock_auto_disabled: bool,
    pub(crate) freefly_toggled: bool,
    pub(crate) zoom_changed: bool,
    pub(crate) last_key: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeCameraState {
    pub(crate) control_mode: CameraControlMode,
    pub(crate) previous_control_mode: CameraControlMode,
    pub(crate) track_enabled: bool,
    pub(crate) active_track_mode: CameraMode,
    pub(crate) saved_track_mode: CameraMode,
}

impl RuntimeCameraState {
    pub(crate) fn new(
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

    pub(crate) fn toggle_freefly(&mut self, has_track_source: bool) -> bool {
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
pub(crate) enum ColorPathLevel {
    Truecolor,
    Q216,
    Mono,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ColorRecoveryState {
    pub(crate) level: ColorPathLevel,
    pub(crate) target_level: ColorPathLevel,
    pub(crate) auto_recover: bool,
    pub(crate) success_streak: u32,
}

impl ColorRecoveryState {
    pub(crate) fn from_requested(
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

    pub(crate) fn set_requested(
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

    pub(crate) fn degrade(&mut self, ascii_force_color_active: bool, mode: RenderMode) -> bool {
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

    pub(crate) fn on_present_success(&mut self) -> bool {
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

    pub(crate) fn apply(
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
pub(crate) struct OrbitState {
    pub(crate) angle: f32,
    pub(crate) speed: f32,
    pub(crate) enabled: bool,
}

impl OrbitState {
    pub(crate) fn new(initial_speed: f32) -> Self {
        Self {
            angle: std::f32::consts::FRAC_PI_2,
            speed: initial_speed.max(0.0),
            enabled: initial_speed > 0.0,
        }
    }

    pub(crate) fn advance(&mut self, dt: f32) {
        if self.enabled && self.speed > 0.0 {
            self.angle += self.speed * dt.max(0.0);
        }
    }
}
pub(crate) fn cap_render_size(width: u16, height: u16) -> (u16, u16, bool) {
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

pub(crate) fn is_terminal_size_unstable(width: u16, height: u16) -> bool {
    if width == 0 || height == 0 {
        return true;
    }
    if width == u16::MAX || height == u16::MAX {
        return true;
    }
    let w = width as u32;
    let h = height as u32;
    let max_w = (MAX_RENDER_COLS as u32) * 8;
    let max_h = (MAX_RENDER_ROWS as u32) * 8;
    w > max_w || h > max_h
}

pub(crate) fn resolve_runtime_backend(requested: RenderBackend) -> RenderBackend {
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

pub(crate) fn normalize_graphics_settings(config: &mut RenderConfig) -> Option<String> {
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

pub(crate) fn format_runtime_status(
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

pub(crate) fn overlay_osd(frame: &mut crate::renderer::FrameBuffers, text: &str) {
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
