use std::path::PathBuf;

use glam::Vec3;

use crate::scene::{
    AnsiQuantization, CameraAlignPreset, CameraControlMode, CameraMode, ColorMode, ContrastProfile,
    RenderMode,
};

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimePmxSettings {
    pub(crate) gravity: Vec3,
    pub(crate) warmup_steps: u32,
    pub(crate) unit_step: f32,
    pub(crate) max_substeps: usize,
}

impl Default for RuntimePmxSettings {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.8, 0.0),
            warmup_steps: 24,
            unit_step: 0.008,
            max_substeps: 8,
        }
    }
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
