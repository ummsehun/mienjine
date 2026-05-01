use std::time::Instant;

use crate::{
    interfaces::cli::terminal_interface::PresentMode,
    runtime::options::color_path_label,
    scene::{RenderConfig, RenderOutputMode},
};

use super::bootstrap::BootstrapState;
use super::helpers::{is_retryable_io_error, resize_runtime_frame};

pub(super) struct PresentResult {
    pub(super) continue_loop: bool,
    pub(super) should_break: bool,
}

pub(super) fn present_frame(
    state: &mut BootstrapState,
    frame_config: &RenderConfig,
    input_resized_or_recovery: bool,
) -> anyhow::Result<PresentResult> {
    if state.active_graphics_protocol.is_some()
        && usize::from(state.display_cells.0).saturating_mul(usize::from(state.display_cells.1))
            > crate::runtime::state::HYBRID_GRAPHICS_MAX_CELLS
    {
        state.active_graphics_protocol = None;
        resize_runtime_frame(
            &mut state.terminal,
            &mut state.frame,
            &state.config,
            state.display_cells,
            false,
        );
        state.terminal.force_full_repaint();
        state.last_osd_notice = Some("graphics fallback: text (terminal too large)".to_owned());
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }

    let present_started = Instant::now();
    let present_result = if let Some(protocol) = state.active_graphics_protocol {
        state.terminal.present_graphics(
            &state.frame,
            protocol,
            frame_config.kitty_transport,
            frame_config.kitty_compression,
            frame_config.kitty_pipeline_mode,
            frame_config.recover_strategy,
            frame_config.kitty_scale,
            state.display_cells,
            input_resized_or_recovery,
        )
    } else if matches!(state.color_mode, crate::scene::ColorMode::Ansi) {
        state
            .terminal
            .present(&state.frame, true, state.ansi_quantization)
    } else {
        state
            .terminal
            .present(&state.frame, false, state.ansi_quantization)
    };

    if let Err(err) = present_result {
        if state.active_graphics_protocol.is_some() {
            if matches!(
                state.config.output_mode,
                RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
            ) {
                state.active_graphics_protocol = None;
                resize_runtime_frame(
                    &mut state.terminal,
                    &mut state.frame,
                    &state.config,
                    state.display_cells,
                    false,
                );
                state.terminal.force_full_repaint();
                state.last_osd_notice = Some("graphics fallback: text".to_owned());
                state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
                return Ok(PresentResult {
                    continue_loop: true,
                    should_break: false,
                });
            }
            return Err(err);
        }

        if is_retryable_io_error(&err) {
            state.io_failure_count = state.io_failure_count.saturating_add(1);
            if state.io_failure_count >= 3 {
                state.io_failure_count = 0;
                state
                    .color_recovery
                    .degrade(state.ascii_force_color_active, frame_config.mode);
                state.color_recovery.apply(
                    &mut state.color_mode,
                    &mut state.ansi_quantization,
                    frame_config.mode,
                    state.ascii_force_color_active,
                );
                state.terminal.set_present_mode(PresentMode::FullFallback);
                state.last_osd_notice = Some(format!(
                    "io fallback: {}",
                    color_path_label(state.color_mode, state.ansi_quantization)
                ));
                state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
            }
            return Ok(PresentResult {
                continue_loop: true,
                should_break: false,
            });
        }
        state.io_failure_count = state.io_failure_count.saturating_add(1);
        if state.io_failure_count >= 3 {
            state.io_failure_count = 0;
            state
                .color_recovery
                .degrade(state.ascii_force_color_active, frame_config.mode);
            state.color_recovery.apply(
                &mut state.color_mode,
                &mut state.ansi_quantization,
                frame_config.mode,
                state.ascii_force_color_active,
            );
            state.terminal.set_present_mode(PresentMode::FullFallback);
            state.last_osd_notice = Some(format!(
                "error fallback: {}",
                color_path_label(state.color_mode, state.ansi_quantization)
            ));
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
            return Ok(PresentResult {
                continue_loop: true,
                should_break: false,
            });
        }
        return Err(err);
    }

    if state.active_graphics_protocol.is_some() {
        let present_ms = present_started.elapsed().as_secs_f32() * 1000.0;
        if present_ms > crate::runtime::state::HYBRID_GRAPHICS_SLOW_FRAME_MS {
            state.graphics_slow_streak = state.graphics_slow_streak.saturating_add(1);
        } else {
            state.graphics_slow_streak = state.graphics_slow_streak.saturating_sub(1);
        }
        if matches!(
            state.config.output_mode,
            RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
        ) && state.graphics_slow_streak
            >= crate::runtime::state::HYBRID_GRAPHICS_SLOW_STREAK_LIMIT
        {
            state.active_graphics_protocol = None;
            state.graphics_slow_streak = 0;
            resize_runtime_frame(
                &mut state.terminal,
                &mut state.frame,
                &state.config,
                state.display_cells,
                false,
            );
            state.terminal.force_full_repaint();
            state.last_osd_notice = Some(format!("graphics fallback: text ({present_ms:.1}ms)"));
            state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
            return Ok(PresentResult {
                continue_loop: true,
                should_break: false,
            });
        }
    } else {
        state.graphics_slow_streak = 0;
    }
    state.io_failure_count = 0;

    Ok(PresentResult {
        continue_loop: false,
        should_break: false,
    })
}

pub(super) fn handle_present_success(state: &mut BootstrapState, frame_config: &RenderConfig) {
    if state.color_recovery.on_present_success() {
        state.color_recovery.apply(
            &mut state.color_mode,
            &mut state.ansi_quantization,
            frame_config.mode,
            state.ascii_force_color_active,
        );
        state.last_osd_notice = Some(format!(
            "color recover: {}",
            color_path_label(state.color_mode, state.ansi_quantization)
        ));
        state.osd_until = Some(Instant::now() + std::time::Duration::from_secs(2));
    }
}
