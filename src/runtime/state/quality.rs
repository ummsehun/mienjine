use glam::Vec3;

use crate::{
    renderer::{Camera, RenderStats},
    scene::{CenterLockMode, ContrastProfile, PerfProfile, RenderConfig, RenderMode},
};

use super::RuntimeContrastPreset;

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeAdaptiveQuality {
    pub(crate) target_frame_ms: f32,
    pub(crate) ema_frame_ms: f32,
    pub(crate) lod_level: usize,
    pub(crate) overload_streak: u32,
    pub(crate) underload_streak: u32,
}

impl RuntimeAdaptiveQuality {
    pub(crate) fn new(profile: PerfProfile) -> Self {
        Self {
            target_frame_ms: target_frame_ms(profile),
            ema_frame_ms: target_frame_ms(profile),
            lod_level: 0,
            overload_streak: 0,
            underload_streak: 0,
        }
    }

    pub(crate) fn observe(&mut self, frame_ms: f32) -> bool {
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
pub(crate) struct VisibilityWatchdog {
    pub(crate) low_visible_streak: u32,
}

impl VisibilityWatchdog {
    pub(crate) fn observe(&mut self, visible_ratio: f32) -> bool {
        if visible_ratio < VISIBILITY_LOW_THRESHOLD {
            self.low_visible_streak = self.low_visible_streak.saturating_add(1);
        } else {
            self.low_visible_streak = 0;
        }
        self.low_visible_streak >= VISIBILITY_LOW_FRAMES_TO_RECOVER
    }

    pub(crate) fn reset(&mut self) {
        self.low_visible_streak = 0;
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct CenterLockState {
    pub(crate) err_x_ema: f32,
    pub(crate) err_y_ema: f32,
}

impl CenterLockState {
    pub(crate) fn apply_camera_space(
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

    pub(crate) fn reset(&mut self) {
        self.err_x_ema = 0.0;
        self.err_y_ema = 0.0;
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScreenFitController {
    pub(crate) auto_zoom_gain: f32,
}

impl Default for ScreenFitController {
    fn default() -> Self {
        Self {
            auto_zoom_gain: 1.0,
        }
    }
}

impl ScreenFitController {
    pub(crate) fn on_resize(&mut self) {
        self.auto_zoom_gain = 1.0;
    }

    pub(crate) fn on_manual_zoom(&mut self) {
        self.auto_zoom_gain = self.auto_zoom_gain.clamp(0.55, 1.80);
    }

    pub(crate) fn target_for_mode(mode: RenderMode) -> f32 {
        match mode {
            RenderMode::Ascii => 0.72,
            RenderMode::Braille => 0.66,
        }
    }

    pub(crate) fn update(&mut self, visible_height_ratio: f32, mode: RenderMode, enabled: bool) {
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
pub(crate) struct ExposureAutoBoost {
    pub(crate) low_streak: u32,
    pub(crate) high_streak: u32,
    pub(crate) boost: f32,
}

impl ExposureAutoBoost {
    pub(crate) fn on_resize(&mut self) {
        self.low_streak = 0;
        self.high_streak = 0;
        self.boost = 0.0;
    }

    pub(crate) fn update(&mut self, visible_ratio: f32) {
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
pub(crate) struct AutoRadiusGuard {
    pub(crate) low_height_streak: u32,
    pub(crate) recover_streak: u32,
    pub(crate) shrink_ratio: f32,
}

impl AutoRadiusGuard {
    pub(crate) fn update(&mut self, height_ratio: f32, enabled: bool) -> f32 {
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
pub(crate) struct DistanceClampGuard {
    pub(crate) last_eye: Option<Vec3>,
}

impl DistanceClampGuard {
    pub(crate) fn apply(
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

    pub(crate) fn reset(&mut self) {
        self.last_eye = None;
    }
}

pub(crate) fn dynamic_clip_planes(
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

pub(crate) fn target_frame_ms(profile: PerfProfile) -> f32 {
    match profile {
        PerfProfile::Balanced => 33.3,
        PerfProfile::Cinematic => 50.0,
        PerfProfile::Smooth => 22.2,
    }
}

pub(crate) fn apply_runtime_contrast_preset(
    config: &mut RenderConfig,
    preset: RuntimeContrastPreset,
) {
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
