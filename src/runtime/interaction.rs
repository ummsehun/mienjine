use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use glam::Vec3;

use crate::{
    runtime::interaction_camera::{freefly_rotate, freefly_translate, FreeFlyDirection},
    runtime::state::{
        is_terminal_size_unstable, RuntimeContrastPreset, RuntimeInputResult, SYNC_OFFSET_LIMIT_MS,
        SYNC_OFFSET_STEP_MS,
    },
    scene::{
        BrailleProfile, CameraControlMode, CinematicCameraMode, ColorMode, FreeFlyState, SceneCpu,
    },
};

pub(crate) use crate::runtime::interaction_camera::{
    freefly_camera, freefly_state_from_camera, orbit_camera, update_camera_director,
};

pub(crate) fn process_runtime_input(
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

pub(crate) fn max_scene_vertices(scene: &SceneCpu) -> usize {
    scene
        .meshes
        .iter()
        .map(|mesh| mesh.positions.len())
        .max()
        .unwrap_or(0)
}
