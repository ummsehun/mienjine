use std::time::{Duration, Instant};

use glam::Vec3;

use crate::{
    renderer::RenderStats,
    runtime::state::{AutoRadiusGuard, CameraDirectorState},
    scene::{BrailleProfile, CinematicCameraMode},
};

use super::bootstrap::BootstrapState;

pub(super) fn handle_post_render_state(state: &mut BootstrapState, stats: RenderStats) {
    state.last_render_stats = stats;
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

    update_camera_tracking(state, &stats, subject_visible_ratio);

    state.auto_radius_guard.update(
        subject_height_ratio,
        state.center_lock_enabled && matches!(state.braille_profile, BrailleProfile::Safe),
    );
    state.screen_fit.update(
        subject_height_ratio,
        state.config.mode,
        state.center_lock_enabled,
    );
    state.exposure_auto_boost.update(subject_visible_ratio);

    if state.visibility_watchdog.observe(stats.visible_cell_ratio) {
        state.visibility_watchdog.reset();
        state.user_zoom = 1.0;
        state.focus_offset = Vec3::ZERO;
        state.camera_height_offset = 0.0;
        state.exposure_bias = (state.exposure_bias + 0.08).clamp(-0.5, 0.8);
        state.center_lock_state.reset();
        state.auto_radius_guard = AutoRadiusGuard::default();
        state.distance_clamp_guard.reset();
        state.screen_fit.on_resize();
        state.exposure_auto_boost.on_resize();
        state.camera_director = CameraDirectorState::default();
        state.cinematic_mode = CinematicCameraMode::On;
        state.last_osd_notice = Some("visibility recover".to_owned());
        state.osd_until = Some(Instant::now() + Duration::from_secs(2));
    }
}

fn update_camera_tracking(
    state: &mut BootstrapState,
    stats: &RenderStats,
    subject_visible_ratio: f32,
) {
    if state.runtime_camera.track_enabled {
        if subject_visible_ratio < 0.0015 {
            state.track_lost_streak = state.track_lost_streak.saturating_add(1);
        } else {
            state.track_lost_streak = 0;
        }

        let vmd_active = state.loaded_camera_track.is_some()
            && !matches!(state.runtime_camera.active_track_mode, crate::scene::CameraMode::Off);

        let centroid = stats.subject_centroid_px.or(stats.visible_centroid_px);
        if state.center_lock_enabled && !vmd_active {
            if let Some((cx, cy)) = centroid {
                let fw = f32::from(state.frame.width.max(1));
                let fh = f32::from(state.frame.height.max(1));
                let nx = ((cx / fw - 0.5) * 2.0).clamp(-2.0, 2.0);
                let ny = ((cy / fh - 0.5) * 2.0).clamp(-2.0, 2.0);
                if nx.abs() > 0.55 || ny.abs() > 0.55 {
                    state.center_drift_streak = state.center_drift_streak.saturating_add(1);
                } else {
                    state.center_drift_streak = 0;
                }
            } else {
                state.center_drift_streak = state.center_drift_streak.saturating_add(1);
            }
        } else {
            state.center_drift_streak = 0;
        }

        if state.center_drift_streak >= 18 {
            state.runtime_camera.track_enabled = false;
            state.center_drift_streak = 0;
            state.track_lost_streak = 0;
            state.center_lock_state.reset();
            state.last_osd_notice = Some(
                "camera track drifted off-center: fallback orbit (toggle f to retry)".to_owned(),
            );
            state.osd_until = Some(Instant::now() + Duration::from_secs(3));
        }
        if state.track_lost_streak >= 24 {
            state.runtime_camera.track_enabled = false;
            state.track_lost_streak = 0;
            state.center_drift_streak = 0;
            state.center_lock_state.reset();
            state.last_osd_notice =
                Some("camera track lost subject: fallback orbit (toggle f to retry)".to_owned());
            state.osd_until = Some(Instant::now() + Duration::from_secs(3));
        }
    } else {
        state.track_lost_streak = 0;
        state.center_drift_streak = 0;
    }
}
