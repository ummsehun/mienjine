mod bootstrap;
mod frame_setup;
mod helpers;
mod input_resize;
mod post_render;
mod present;

use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use glam::Vec3;

use crate::{
    render::backend::render_frame_with_backend,
    runtime::{
        audio_sync::compute_animation_time,
        interaction::{freefly_camera, orbit_camera, update_camera_director},
        options::RuntimeSyncProfileContext,
        state::{
            RuntimeCameraSettings, RuntimePmxSettings, dynamic_clip_planes, format_runtime_status,
            jitter_scale_for_lod, overlay_osd,
        },
        sync_profile::SyncProfileMode,
    },
    scene::{
        AudioReactiveMode, CameraControlMode, CameraMode, CellAspectMode, RenderConfig,
        RenderOutputMode, SceneCpu, resolve_cell_aspect,
    },
};

use bootstrap::bootstrap_runtime;
use frame_setup::build_frame_config;
use helpers::set_runtime_panic_state_proxy as set_runtime_panic_state;
use input_resize::{
    handle_frame_resize, handle_input_notices, handle_resize_recovery, process_frame_input,
};
use post_render::handle_post_render_state;
use present::{handle_present_success, present_frame};

pub(crate) fn run_scene_interactive(
    scene: SceneCpu,
    animation_index: Option<usize>,
    rotates_without_animation: bool,
    config: RenderConfig,
    audio_sync: Option<crate::runtime::audio_sync::AudioSyncRuntime>,
    initial_sync_offset_ms: i32,
    orbit_speed: f32,
    orbit_radius: f32,
    camera_height: f32,
    look_at_y: f32,
    wasd_mode: CameraControlMode,
    freefly_speed: f32,
    camera_settings: RuntimeCameraSettings,
    pmx_settings: RuntimePmxSettings,
    sync_profile: Option<RuntimeSyncProfileContext>,
) -> Result<()> {
    let mut state = bootstrap_runtime(
        scene,
        animation_index,
        rotates_without_animation,
        config,
        audio_sync,
        initial_sync_offset_ms,
        orbit_speed,
        orbit_radius,
        camera_height,
        look_at_y,
        wasd_mode,
        freefly_speed,
        camera_settings.clone(),
        pmx_settings,
        sync_profile,
    )?;

    if let Some(audio) = state.audio_sync.as_ref() {
        audio.playback.sink.play();
    }

    let clip_duration = state
        .animation_index
        .and_then(|idx| state.scene.animations.get(idx))
        .map(|clip| clip.duration)
        .filter(|duration| *duration > f32::EPSILON);

    let camera_settings_look_speed = camera_settings.look_speed;

    loop {
        let frame_start = Instant::now();

        let input = process_frame_input(&mut state, camera_settings_look_speed)?;
        if input.quit {
            break;
        }

        handle_frame_resize(&mut state, &input);

        if handle_resize_recovery(&mut state) {
            continue;
        }

        handle_input_notices(&mut state, &input);

        let elapsed_wall = state.start.elapsed().as_secs_f32();
        let dt = (elapsed_wall - state.prev_wall_seconds).max(0.0);
        state.prev_wall_seconds = elapsed_wall;
        state.sim_accum = (state.sim_accum + dt).min(0.25);
        while state.sim_accum >= state.fixed_step {
            state.sim_time += state.fixed_step;
            state.sim_accum -= state.fixed_step;
        }
        state.orbit_state.advance(dt);
        let sync_speed = state
            .audio_sync
            .as_ref()
            .map(|s| s.speed_factor)
            .unwrap_or(1.0);
        let elapsed_audio = state
            .audio_sync
            .as_ref()
            .map(|s| s.playback.sink.get_pos().as_secs_f32());
        let raw_energy = if matches!(state.config.audio_reactive, AudioReactiveMode::Off) {
            0.0
        } else {
            state
                .audio_sync
                .as_ref()
                .and_then(|sync| {
                    elapsed_audio
                        .and_then(|audio_time| sync.envelope.as_ref().map(|e| e.sample(audio_time)))
                })
                .unwrap_or(0.0)
        };
        state.reactive_state.energy = raw_energy;
        state.reactive_state.smoothed_energy +=
            (raw_energy - state.reactive_state.smoothed_energy) * 0.18;
        let interpolated_wall =
            state.sim_time + state.sim_accum / state.fixed_step * state.fixed_step;
        let animation_time = compute_animation_time(
            &mut state.continuous_sync_state,
            state.config.sync_policy,
            dt,
            interpolated_wall,
            elapsed_audio,
            sync_speed,
            state.sync_offset_ms,
            state.config.sync_hard_snap_ms,
            state.config.sync_kp,
            clip_duration,
        );
        if clip_duration.is_some()
            && state
                .prev_animation_time
                .is_some_and(|previous| animation_time + 1e-4 < previous)
            && let Some(physics_state) = state.pmx_physics_state.as_mut()
        {
            physics_state.reset(&state.scene);
        }
        state.prev_animation_time = Some(animation_time);
        let scene = &state.scene;
        let physics_state = state
            .pmx_physics_state
            .as_mut()
            .map(|physics| physics as &mut dyn crate::engine::pipeline::PhysicsStepper);
        state.pipeline.prepare_frame(
            scene,
            animation_time,
            state.animation_index,
            physics_state,
            dt,
        );
        let rotation = if state.animation_index.is_some() {
            0.0
        } else if state.model_spin_enabled {
            elapsed_wall * 0.9
        } else {
            0.0
        };
        if state.animation_index.is_none() && rotation.abs() > 0.0 {
            state.terminal.force_full_repaint();
        }
        let detected_cell_aspect = if state.config.cell_aspect_mode == CellAspectMode::Auto {
            helpers::detect_terminal_cell_aspect()
        } else {
            None
        };
        let target_aspect = resolve_cell_aspect(&state.config, detected_cell_aspect);
        state.effective_aspect_state += (target_aspect - state.effective_aspect_state) * 0.22;
        let effective_aspect = state.effective_aspect_state.clamp(0.30, 1.20);
        if state.active_graphics_protocol.is_some()
            && matches!(state.config.output_mode, RenderOutputMode::KittyHq)
        {
            let target_internal = helpers::kitty_internal_res_for_lod(
                state.kitty_internal_res_base,
                state.adaptive_quality.lod_level,
            );
            if state.config.kitty_internal_res != target_internal {
                state.config.kitty_internal_res = target_internal;
                state.render_cells = helpers::resize_runtime_frame(
                    &mut state.terminal,
                    &mut state.frame,
                    &state.config,
                    state.display_cells,
                    true,
                );
                state.last_osd_notice = Some(format!(
                    "kitty-hq adapt: internal={}x{} (lod={})",
                    state.render_cells.0, state.render_cells.1, state.adaptive_quality.lod_level
                ));
                state.osd_until = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        let mut frame_config = build_frame_config(&mut state, effective_aspect);

        let jitter_scale = jitter_scale_for_lod(state.adaptive_quality.lod_level);
        let (radius_mul, height_off, focus_y_off, angle_jitter) = update_camera_director(
            &mut state.camera_director,
            state.cinematic_mode,
            state.camera_focus_mode,
            elapsed_wall,
            state.reactive_state.smoothed_energy,
            state.reactive_gain,
            state.framing.extent_y,
            jitter_scale,
        );
        let effective_zoom = (state.user_zoom * state.screen_fit.auto_zoom_gain).clamp(0.20, 8.0);
        let zoom_repaint_threshold = if state.terminal_profile.is_ghostty {
            0.20
        } else {
            0.12
        };
        let repaint_due_to_zoom = (effective_zoom - 1.0).abs() > zoom_repaint_threshold
            || state.focus_offset.length_squared() > 0.01
            || state.camera_height_offset.abs() > 0.01;
        if repaint_due_to_zoom {
            if state.terminal_profile.is_ghostty {
                state.ghostty_zoom_repaint_due = Some(Instant::now() + Duration::from_millis(45));
            } else {
                state.terminal.force_full_repaint();
            }
        }
        if state.terminal_profile.is_ghostty
            && state
                .ghostty_zoom_repaint_due
                .is_some_and(|due| Instant::now() >= due)
        {
            state.terminal.force_full_repaint();
            state.ghostty_zoom_repaint_due = None;
        }
        let auto_radius_shrink = state.auto_radius_guard.shrink_ratio;
        if !state.center_lock_enabled
            || matches!(
                state.runtime_camera.control_mode,
                CameraControlMode::FreeFly
            )
        {
            state.center_lock_state.reset();
        }
        let mut camera = if matches!(
            state.runtime_camera.control_mode,
            CameraControlMode::FreeFly
        ) {
            freefly_camera(state.freefly_state)
        } else {
            orbit_camera(
                state.orbit_state.angle + angle_jitter,
                (state.framing.radius * effective_zoom * radius_mul * (1.0 - auto_radius_shrink))
                    .clamp(0.2, 1000.0),
                (state.framing.camera_height + state.camera_height_offset + height_off)
                    .clamp(-1000.0, 1000.0),
                state.framing.focus
                    + if state.center_lock_enabled {
                        Vec3::ZERO
                    } else {
                        state.focus_offset
                    }
                    + Vec3::new(
                        0.0,
                        if state.center_lock_enabled {
                            focus_y_off.clamp(
                                -state.framing.extent_y * 0.03,
                                state.framing.extent_y * 0.03,
                            )
                        } else {
                            focus_y_off
                        },
                        0.0,
                    ),
            )
        };
        if state.runtime_camera.track_enabled
            && let Some(track) = state.loaded_camera_track.as_ref()
            && let Some(vmd_pose) = track
                .sampler
                .sample_pose(animation_time, track.transform, true)
        {
            match state.runtime_camera.active_track_mode {
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
                    frame_config.fov_deg = frame_config.fov_deg * 0.30 + vmd_pose.fov_deg * 0.70;
                }
            }
        }
        let subject_target = if let Some(node_index) = state.scene.root_center_node {
            state
                .pipeline
                .globals()
                .get(node_index)
                .copied()
                .unwrap_or(glam::Mat4::IDENTITY)
                .transform_point3(Vec3::ZERO)
        } else {
            state.framing.focus
        };
        if state.center_lock_enabled
            && !matches!(
                state.runtime_camera.control_mode,
                CameraControlMode::FreeFly
            )
        {
            state.center_lock_state.apply_camera_space(
                &state.last_render_stats,
                state.center_lock_mode,
                state.frame.width,
                state.frame.height,
                &mut camera,
                frame_config.fov_deg,
                frame_config.cell_aspect,
                state.framing.extent_y,
            );
        }
        let min_dist = state.distance_clamp_guard.apply(
            &mut camera,
            subject_target,
            state.framing.extent_y,
            0.35,
        );
        let camera_dist = (camera.eye - subject_target).length().max(min_dist);
        let (dyn_near, dyn_far) = dynamic_clip_planes(
            min_dist,
            state.framing.extent_y,
            camera_dist,
            state.framing.has_stage_mesh,
        );
        frame_config.near = dyn_near;
        frame_config.far = dyn_far;

        let stats = render_frame_with_backend(
            &mut state.gpu_renderer_state,
            &mut state.frame,
            &frame_config,
            &state.scene,
            state.pipeline.globals(),
            state.pipeline.skin_matrices(),
            state.pipeline.morph_weights_by_instance(),
            state.pipeline.material_morph_weights(),
            &state.glyph_ramp,
            &mut state.render_scratch,
            camera,
            rotation,
        );
        handle_post_render_state(&mut state, stats);

        let work_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        if state.adaptive_quality.observe(work_ms) {
            state.last_osd_notice = Some(format!(
                "lod={} target={:.1}ms ema={:.1}ms",
                state.adaptive_quality.lod_level,
                state.adaptive_quality.target_frame_ms,
                state.adaptive_quality.ema_frame_ms
            ));
            state.osd_until = Some(Instant::now() + Duration::from_secs(2));
        }

        if state.osd_until.is_some_and(|until| Instant::now() <= until) {
            let status = format_runtime_status(
                state.sync_offset_ms,
                sync_speed,
                effective_aspect,
                state.contrast_preset,
                frame_config.braille_profile,
                frame_config.color_mode,
                frame_config.cinematic_camera,
                frame_config.reactive_gain,
                frame_config.exposure_bias,
                frame_config.stage_level,
                frame_config.center_lock,
                state.adaptive_quality.lod_level,
                state.adaptive_quality.target_frame_ms,
                state.adaptive_quality.ema_frame_ms,
                state.sync_profile.as_ref().map(|profile| profile.hit),
                state.sync_profile_dirty,
                state.continuous_sync_state.drift_ema,
                state.continuous_sync_state.hard_snap_count,
                state.last_osd_notice.as_deref(),
            );
            overlay_osd(&mut state.frame, &status);
        }

        let resize_pending = state.resize_recovery_pending;
        let present_result =
            present_frame(&mut state, &frame_config, input.resized || resize_pending)?;

        if present_result.continue_loop {
            continue;
        }
        if present_result.should_break {
            break;
        }

        handle_present_success(&mut state, &frame_config);

        if input.last_key.is_some() {
            state.last_osd_notice = None;
        }

        set_runtime_panic_state(format!(
            "mode={:?} backend={:?} size={}x{} fps_cap={} key={} lod={}",
            frame_config.mode,
            frame_config.backend,
            state.frame.width,
            state.frame.height,
            frame_config.fps_cap,
            input.last_key.unwrap_or("-".to_string()),
            state.adaptive_quality.lod_level
        ));

        let elapsed_frame = frame_start.elapsed();
        if let Some(frame_budget) = state.frame_budget
            && elapsed_frame < frame_budget
        {
            thread::sleep(frame_budget - elapsed_frame);
        }
    }
    if let Some(profile) = state.sync_profile.as_ref()
        && state.sync_profile_dirty
        && matches!(profile.mode, SyncProfileMode::Auto | SyncProfileMode::Write)
        && let Err(err) =
            crate::runtime::app::persist_sync_profile_offset(profile, state.sync_offset_ms)
    {
        eprintln!(
            "warning: failed to save sync profile {}: {err}",
            profile.store_path.display()
        );
    }
    crate::runtime::graphics_proto::cleanup_shm_registry();
    Ok(())
}
