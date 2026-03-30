use std::time::Instant;

use crate::{
    runtime::{interaction::process_runtime_input, state::is_terminal_size_unstable},
    scene::{ColorMode, RenderOutputMode},
};

use super::bootstrap::BootstrapState;

pub(super) struct InputResult {
    pub(super) quit: bool,
    pub(super) resized: bool,
    pub(super) resized_terminal: Option<(u16, u16)>,
    pub(super) terminal_size_unstable: bool,
    pub(super) status_changed: bool,
    pub(super) stage_changed: bool,
    pub(super) center_lock_blocked_pan: bool,
    pub(super) center_lock_auto_disabled: bool,
    pub(super) zoom_changed: bool,
    pub(super) freefly_toggled: bool,
    pub(super) last_key: Option<String>,
}

pub(super) fn process_frame_input(
    state: &mut BootstrapState,
    camera_settings_look_speed: f32,
) -> anyhow::Result<InputResult> {
    let sync_offset_before_input = state.sync_offset_ms;
    let input = process_runtime_input(
        &mut state.orbit_state.enabled,
        &mut state.orbit_state.speed,
        &mut state.model_spin_enabled,
        &mut state.user_zoom,
        &mut state.focus_offset,
        &mut state.camera_height_offset,
        &mut state.center_lock_enabled,
        &mut state.stage_level,
        &mut state.sync_offset_ms,
        &mut state.contrast_preset,
        &mut state.braille_profile,
        &mut state.color_mode,
        &mut state.cinematic_mode,
        &mut state.reactive_gain,
        &mut state.exposure_bias,
        &mut state.runtime_camera.control_mode,
        camera_settings_look_speed,
        &mut state.freefly_state,
    )?;

    if state.sync_offset_ms != sync_offset_before_input {
        state.sync_profile_dirty = true;
        if state.sync_profile.is_some() {
            state.last_osd_notice = Some(format!(
                "sync profile dirty: offset={}ms",
                state.sync_offset_ms
            ));
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
        }
    }

    Ok(InputResult {
        quit: input.quit,
        resized: input.resized,
        resized_terminal: input.resized_terminal,
        terminal_size_unstable: input.terminal_size_unstable,
        status_changed: input.status_changed,
        stage_changed: input.stage_changed,
        center_lock_blocked_pan: input.center_lock_blocked_pan,
        center_lock_auto_disabled: input.center_lock_auto_disabled,
        zoom_changed: input.zoom_changed,
        freefly_toggled: input.freefly_toggled,
        last_key: input.last_key.map(String::from),
    })
}

pub(super) fn handle_frame_resize(state: &mut BootstrapState, input: &InputResult) {
    if input.resized {
        state.terminal.force_full_repaint();
        state.distance_clamp_guard.reset();
        state.screen_fit.on_resize();
        state.exposure_auto_boost.on_resize();
        state.last_render_stats = crate::renderer::RenderStats::default();
        state.render_scratch.reset_exposure();
        if input.terminal_size_unstable {
            state.resize_recovery_pending = true;
            state.center_lock_state.reset();
            state.last_osd_notice =
                Some("resize unstable: waiting for terminal recovery".to_owned());
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
            std::thread::sleep(std::time::Duration::from_millis(16));
        } else {
            state.resize_recovery_pending = false;
            if let Some((tw, th)) = input.resized_terminal {
                let (rw, rh, _) = crate::runtime::state::cap_render_size(tw, th);
                state.display_cells = (rw.max(1), rh.max(1));
            } else if let Ok((tw, th)) = state.terminal.size() {
                let (rw, rh, _) = crate::runtime::state::cap_render_size(tw, th);
                state.display_cells = (rw.max(1), rh.max(1));
            }
            if matches!(
                state.config.output_mode,
                RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
            ) && state.active_graphics_protocol.is_some()
                && (state.display_cells.0 < 72
                    || state.display_cells.1 < 20
                    || usize::from(state.display_cells.0)
                        .saturating_mul(usize::from(state.display_cells.1))
                        > crate::runtime::state::HYBRID_GRAPHICS_MAX_CELLS)
            {
                state.active_graphics_protocol = None;
                state.last_osd_notice =
                    Some("graphics fallback: text (resize/small terminal safeguard)".to_owned());
                state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
            }
            state.render_cells = super::helpers::resize_runtime_frame(
                &mut state.terminal,
                &mut state.frame,
                &state.config,
                state.display_cells,
                state.active_graphics_protocol.is_some(),
            );
            if state.active_graphics_protocol.is_some() {
                state.last_osd_notice = Some(format!(
                    "resize: display={}x{} render={}x{}",
                    state.display_cells.0,
                    state.display_cells.1,
                    state.render_cells.0,
                    state.render_cells.1
                ));
                state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
            }
        }
    }
}

pub(super) fn handle_resize_recovery(state: &mut BootstrapState) -> bool {
    if !state.resize_recovery_pending {
        return false;
    }
    match state.terminal.size() {
        Ok((tw, th)) if !is_terminal_size_unstable(tw, th) => {
            let (rw, rh, _) = crate::runtime::state::cap_render_size(tw, th);
            state.display_cells = (rw.max(1), rh.max(1));
            if matches!(
                state.config.output_mode,
                RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
            ) && state.active_graphics_protocol.is_some()
                && (state.display_cells.0 < 72
                    || state.display_cells.1 < 20
                    || usize::from(state.display_cells.0)
                        .saturating_mul(usize::from(state.display_cells.1))
                        > crate::runtime::state::HYBRID_GRAPHICS_MAX_CELLS)
            {
                state.active_graphics_protocol = None;
            }
            state.render_cells = super::helpers::resize_runtime_frame(
                &mut state.terminal,
                &mut state.frame,
                &state.config,
                state.display_cells,
                state.active_graphics_protocol.is_some(),
            );
            state.distance_clamp_guard.reset();
            state.screen_fit.on_resize();
            state.exposure_auto_boost.on_resize();
            state.render_scratch.reset_exposure();
            state.last_render_stats = crate::renderer::RenderStats::default();
            state.resize_recovery_pending = false;
            state.last_osd_notice = Some(format!(
                "resize recovered: display={}x{} render={}x{}",
                state.display_cells.0,
                state.display_cells.1,
                state.render_cells.0,
                state.render_cells.1
            ));
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
            true
        }
        _ => {
            state.center_lock_state.reset();
            state.last_osd_notice =
                Some("resize unstable: waiting for terminal recovery".to_owned());
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
            std::thread::sleep(std::time::Duration::from_millis(16));
            false
        }
    }
}

pub(super) fn handle_input_notices(state: &mut BootstrapState, input: &InputResult) {
    if input.status_changed {
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
    if input.stage_changed {
        state.last_osd_notice = Some(format!("stage={}", state.stage_level));
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
    if input.center_lock_blocked_pan {
        state.last_osd_notice = Some("center-lock on: pan disabled (press t to unlock)".to_owned());
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
    if input.center_lock_auto_disabled {
        state.last_osd_notice = Some("center-lock off: freefly active".to_owned());
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
    if input.zoom_changed {
        state.screen_fit.on_manual_zoom();
    }
    if input.freefly_toggled {
        let entered_freefly = state
            .runtime_camera
            .toggle_freefly(state.loaded_camera_track.is_some());
        if entered_freefly {
            state.center_lock_restore_after_freefly = state.center_lock_enabled;
            if state.center_lock_enabled {
                state.center_lock_enabled = false;
                state.center_lock_state.reset();
            }
            state.last_osd_notice = Some("freefly on (track paused)".to_owned());
        } else {
            if state.center_lock_restore_after_freefly && !state.center_lock_enabled {
                state.center_lock_enabled = true;
                state.center_lock_state.reset();
            }
            state.last_osd_notice = Some(if state.runtime_camera.track_enabled {
                if state.center_lock_enabled {
                    "freefly off (track resumed, center-lock restored)".to_owned()
                } else {
                    "freefly off (track resumed)".to_owned()
                }
            } else {
                if state.center_lock_enabled {
                    "freefly off (center-lock restored)".to_owned()
                } else {
                    "freefly off".to_owned()
                }
            });
        }
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
    if input.last_key.as_deref() == Some("c") {
        state.freefly_state = state.initial_freefly_state;
    }
    if matches!(state.config.mode, crate::scene::RenderMode::Ascii)
        && state.ascii_force_color_active
    {
        if input.last_key.as_deref() == Some("n") {
            state.last_osd_notice = Some("ascii color is forced: ansi".to_owned());
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
        }
        state.color_mode = ColorMode::Ansi;
    }
    if input.last_key.as_deref() == Some("n") {
        state.requested_color_mode = crate::runtime::options::resolve_effective_color_mode(
            state.config.mode,
            state.color_mode,
            state.ascii_force_color_active,
        );
        state
            .color_recovery
            .set_requested(state.requested_color_mode, state.ansi_quantization);
        state.color_recovery.apply(
            &mut state.color_mode,
            &mut state.ansi_quantization,
            state.config.mode,
            state.ascii_force_color_active,
        );
    }
}
