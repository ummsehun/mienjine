use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use glam::Vec3;

use crate::{
    pipeline::FramePipeline,
    render::backend::render_frame_with_backend,
    renderer::{FrameBuffers, GlyphRamp, RenderScratch, RenderStats},
    runtime::{
        audio_sync::{compute_animation_time, AudioSyncRuntime},
        graphics_proto::detect_supported_protocol,
        interaction::{
            freefly_camera, freefly_state_from_camera, max_scene_vertices, orbit_camera,
            process_runtime_input, update_camera_director,
        },
        options::{color_path_label, resolve_effective_color_mode, RuntimeSyncProfileContext},
        scene_analysis::compute_scene_framing,
        state::{
            apply_adaptive_quality_tuning, apply_distant_subject_clarity_boost,
            apply_face_focus_detail_boost, apply_pmx_surface_guardrails,
            apply_runtime_contrast_preset, cap_render_size, dynamic_clip_planes,
            format_runtime_status, is_terminal_size_unstable, jitter_scale_for_lod,
            normalize_graphics_settings, overlay_osd, resolve_runtime_backend, AutoRadiusGuard,
            CameraDirectorState, CenterLockState, ColorRecoveryState, ContinuousSyncState,
            DistanceClampGuard, ExposureAutoBoost, OrbitState, ReactiveState,
            RuntimeAdaptiveQuality, RuntimeCameraSettings, RuntimeCameraState,
            RuntimeContrastPreset, ScreenFitController, VisibilityWatchdog,
            HYBRID_GRAPHICS_MAX_CELLS, HYBRID_GRAPHICS_SLOW_FRAME_MS,
            HYBRID_GRAPHICS_SLOW_STREAK_LIMIT, SYNC_OFFSET_LIMIT_MS,
        },
        sync_profile::SyncProfileMode,
    },
    scene::{
        resolve_cell_aspect, AudioReactiveMode, BrailleProfile, CameraControlMode, CameraMode,
        CellAspectMode, CinematicCameraMode, ColorMode, MeshLayer, RenderConfig, RenderMode,
        RenderOutputMode, SceneCpu, StageRole,
    },
    terminal::{PresentMode, TerminalProfile, TerminalSession},
};

mod helpers;

use helpers::{
    detect_terminal_cell_aspect, is_retryable_io_error, kitty_internal_res_for_lod,
    resize_runtime_frame, set_runtime_panic_state_proxy as set_runtime_panic_state,
    validated_terminal_size,
};

pub(crate) fn run_scene_interactive(
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
    let framing = compute_scene_framing(
        &scene,
        config.fov_deg,
        orbit_radius,
        camera_height,
        look_at_y,
    );
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
    let is_pmx_scene = scene.pmx_rig_meta.is_some();
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
    let loaded_camera_track = crate::runtime::audio_sync::load_camera_track(&camera_settings);
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
        apply_pmx_surface_guardrails(&mut frame_config, is_pmx_scene, prev_subject_height_ratio);

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
            pipeline.material_morph_weights(),
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
            if let Err(err) =
                crate::runtime::app::persist_sync_profile_offset(profile, sync_offset_ms)
            {
                eprintln!(
                    "warning: failed to save sync profile {}: {err}",
                    profile.store_path.display()
                );
            }
        }
    }
    crate::runtime::graphics_proto::cleanup_shm_registry();
    Ok(())
}
