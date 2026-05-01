use glam::Vec3;

use crate::{
    renderer::Camera,
    runtime::state::{CameraDirectorState, CameraShot},
    scene::{CameraFocusMode, CinematicCameraMode, FreeFlyState},
};

pub(crate) fn update_camera_director(
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
        CinematicCameraMode::On => 0.15,
        CinematicCameraMode::Aggressive => 0.4,
        CinematicCameraMode::Off => 0.0,
    };
    let jitter = (state.jitter_phase * 0.8).sin()
        * 0.008
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
pub(crate) enum FreeFlyDirection {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

pub(crate) fn freefly_state_from_camera(camera: Camera, move_speed: f32) -> FreeFlyState {
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

pub(crate) fn freefly_camera(state: FreeFlyState) -> Camera {
    Camera {
        eye: state.eye,
        target: state.target,
        up: Vec3::Y,
    }
}

pub(crate) fn freefly_translate(state: &mut FreeFlyState, direction: FreeFlyDirection) {
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

pub(crate) fn freefly_rotate(state: &mut FreeFlyState, yaw_delta: f32, pitch_delta: f32) {
    state.yaw += yaw_delta;
    state.pitch = (state.pitch + pitch_delta).clamp(-1.45, 1.45);
    let forward = freefly_forward(state);
    if forward.length_squared() <= f32::EPSILON {
        return;
    }
    let distance = (state.target - state.eye).length().max(0.5);
    state.target = state.eye + forward * distance;
}

pub(crate) fn orbit_camera(
    orbit_angle: f32,
    orbit_radius: f32,
    camera_height: f32,
    focus: Vec3,
) -> Camera {
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
