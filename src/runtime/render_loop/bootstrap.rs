//! Bootstrap module for initial runtime state preparation.
//!
//! Handles terminal setup, camera initialization, and runtime state setup
//! that occurs before the main frame loop begins.

use std::time::Instant;

use anyhow::Result;
use glam::Vec3;

use crate::{
    pipeline::FramePipeline,
    renderer::{FrameBuffers, GlyphRamp, RenderScratch, RenderStats},
    runtime::{
        audio_sync::{AudioSyncRuntime, LoadedCameraTrack},
        graphics_proto::detect_supported_protocol,
        interaction::{freefly_state_from_camera, max_scene_vertices, orbit_camera},
        options::{resolve_effective_color_mode, RuntimeSyncProfileContext},
        scene_analysis::compute_scene_framing,
        state::{
            cap_render_size, normalize_graphics_settings, resolve_runtime_backend,
            ColorRecoveryState, ContinuousSyncState, ExposureAutoBoost, RuntimeAdaptiveQuality,
            RuntimeCameraSettings, RuntimeCameraState, RuntimeContrastPreset, ScreenFitController,
            HYBRID_GRAPHICS_MAX_CELLS, SYNC_OFFSET_LIMIT_MS,
        },
    },
    scene::{
        resolve_cell_aspect, CameraControlMode, RenderConfig, RenderOutputMode, SceneCpu, StageRole,
    },
    terminal::{PresentMode, TerminalProfile, TerminalSession},
};

use super::helpers::{detect_terminal_cell_aspect, resize_runtime_frame, validated_terminal_size};

/// Initial bootstrap state collected before entering the frame loop.
pub(super) struct BootstrapState {
    pub(super) terminal: TerminalSession,
    pub(super) frame: FrameBuffers,
    pub(super) pipeline: FramePipeline,
    pub(super) pmx_physics_state: Option<crate::runtime::state::PmxPhysicsState>,
    pub(super) glyph_ramp: GlyphRamp,
    pub(super) render_scratch: RenderScratch,
    pub(super) display_cells: (u16, u16),
    pub(super) render_cells: (u16, u16),
    pub(super) active_graphics_protocol: Option<crate::scene::GraphicsProtocol>,
    pub(super) framing: SceneFraming,
    pub(super) orbit_state: crate::runtime::state::OrbitState,
    pub(super) model_spin_enabled: bool,
    pub(super) user_zoom: f32,
    pub(super) focus_offset: Vec3,
    pub(super) camera_height_offset: f32,
    pub(super) center_lock_enabled: bool,
    pub(super) center_lock_mode: crate::scene::CenterLockMode,
    pub(super) stage_level: u8,
    pub(super) requested_color_mode: crate::scene::ColorMode,
    pub(super) ascii_force_color_active: bool,
    pub(super) color_mode: crate::scene::ColorMode,
    pub(super) ansi_quantization: crate::scene::AnsiQuantization,
    pub(super) braille_profile: crate::scene::BrailleProfile,
    pub(super) cinematic_mode: crate::scene::CinematicCameraMode,
    pub(super) camera_focus_mode: crate::scene::CameraFocusMode,
    pub(super) reactive_gain: f32,
    pub(super) exposure_bias: f32,
    pub(super) sync_offset_ms: i32,
    pub(super) contrast_preset: RuntimeContrastPreset,
    pub(super) reactive_state: crate::runtime::state::ReactiveState,
    pub(super) camera_director: crate::runtime::state::CameraDirectorState,
    pub(super) adaptive_quality: RuntimeAdaptiveQuality,
    pub(super) visibility_watchdog: crate::runtime::state::VisibilityWatchdog,
    pub(super) center_lock_state: crate::runtime::state::CenterLockState,
    pub(super) auto_radius_guard: crate::runtime::state::AutoRadiusGuard,
    pub(super) distance_clamp_guard: crate::runtime::state::DistanceClampGuard,
    pub(super) screen_fit: ScreenFitController,
    pub(super) exposure_auto_boost: ExposureAutoBoost,
    pub(super) is_pmx_scene: bool,
    pub(super) base_triangle_stride: usize,
    pub(super) base_min_triangle_area_px2: f32,
    pub(super) center_lock_restore_after_freefly: bool,
    pub(super) initial_freefly_state: crate::scene::FreeFlyState,
    pub(super) loaded_camera_track: Option<LoadedCameraTrack>,
    pub(super) runtime_camera: RuntimeCameraState,
    pub(super) color_recovery: ColorRecoveryState,
    pub(super) continuous_sync_state: ContinuousSyncState,
    pub(super) graphics_slow_streak: u32,
    pub(super) track_lost_streak: u32,
    pub(super) center_drift_streak: u32,
    pub(super) resize_recovery_pending: bool,
    pub(super) sync_profile_dirty: bool,
    pub(super) last_osd_notice: Option<String>,
    pub(super) osd_until: Option<Instant>,
    pub(super) last_render_stats: RenderStats,
    pub(super) effective_aspect_state: f32,
    pub(super) freefly_state: crate::scene::FreeFlyState,
    pub(super) gpu_renderer_state: crate::render::backend_gpu::GpuRendererState,
    pub(super) io_failure_count: u8,
    pub(super) ghostty_zoom_repaint_due: Option<Instant>,
    pub(super) kitty_internal_res_base: crate::scene::KittyInternalResPreset,
    pub(super) frame_budget: Option<std::time::Duration>,
    pub(super) fixed_step: f32,
    pub(super) sim_time: f32,
    pub(super) sim_accum: f32,
    pub(super) prev_wall_seconds: f32,
    pub(super) start: Instant,
    pub(super) terminal_profile: TerminalProfile,
    pub(super) sync_profile: Option<RuntimeSyncProfileContext>,
    pub(super) animation_index: Option<usize>,
    pub(super) scene: SceneCpu,
    pub(super) config: RenderConfig,
    pub(super) audio_sync: Option<AudioSyncRuntime>,
}

/// Scene framing parameters computed during bootstrap.
pub(super) struct SceneFraming {
    pub(super) radius: f32,
    pub(super) camera_height: f32,
    pub(super) focus: Vec3,
    pub(super) extent_y: f32,
    pub(super) has_stage_mesh: bool,
}

/// Bootstrap the runtime state and return the collected state.
pub(super) fn bootstrap_runtime(
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
) -> Result<BootstrapState> {
    config.backend = resolve_runtime_backend(config.backend);
    let startup_graphics_notice = normalize_graphics_settings(&mut config);
    let terminal_profile = TerminalProfile::detect();
    let _truecolor_supported = terminal_profile.supports_truecolor;
    let mut terminal = TerminalSession::enter_with_profile(terminal_profile)?;
    terminal.set_present_mode(PresentMode::Diff);
    let (term_width, term_height) = validated_terminal_size(&terminal)?;
    let (display_width, display_height, scaled) = cap_render_size(term_width, term_height);
    let display_cells = (display_width, display_height);
    let mut frame = FrameBuffers::new(display_width, display_height);
    if scaled {
        eprintln!(
            "info: terminal size {}x{} capped to internal render {}x{}",
            term_width, term_height, display_width, display_height
        );
    }
    let pipeline = FramePipeline::new(&scene);
    let pmx_physics_state = crate::runtime::state::PmxPhysicsState::from_scene(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
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
        .filter(|instance| matches!(instance.layer, crate::scene::MeshLayer::Subject))
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
        .any(|instance| matches!(instance.layer, crate::scene::MeshLayer::Stage))
        && !matches!(config.stage_role, StageRole::Off);
    let orbit_state = crate::runtime::state::OrbitState::new(orbit_speed);
    let model_spin_enabled = rotates_without_animation;
    let user_zoom = 1.0_f32;
    let focus_offset = Vec3::ZERO;
    let camera_height_offset = 0.0_f32;
    let center_lock_enabled = config.center_lock;
    let center_lock_mode = config.center_lock_mode;
    let stage_level = config.stage_level.min(4);
    let gpu_renderer_state = crate::render::backend_gpu::GpuRendererState::default();
    let requested_color_mode =
        resolve_effective_color_mode(config.mode, config.color_mode, config.ascii_force_color);
    let ascii_force_color_active = config.ascii_force_color;
    let braille_profile = config.braille_profile;
    let cinematic_mode = config.cinematic_camera;
    let camera_focus_mode = config.camera_focus;
    let reactive_gain = config.reactive_gain.clamp(0.0, 1.0);
    let exposure_bias = config.exposure_bias.clamp(-0.5, 0.8);
    let sync_offset_ms = initial_sync_offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
    let contrast_preset = RuntimeContrastPreset::from_profile(config.contrast_profile);
    let reactive_state = crate::runtime::state::ReactiveState::default();
    let camera_director = crate::runtime::state::CameraDirectorState::default();
    let adaptive_quality = RuntimeAdaptiveQuality::new(config.perf_profile);
    let visibility_watchdog = crate::runtime::state::VisibilityWatchdog::default();
    let center_lock_state = crate::runtime::state::CenterLockState::default();
    let auto_radius_guard = crate::runtime::state::AutoRadiusGuard::default();
    let distance_clamp_guard = crate::runtime::state::DistanceClampGuard::default();
    let screen_fit = ScreenFitController::default();
    let exposure_auto_boost = ExposureAutoBoost::default();
    let is_pmx_scene = scene.pmx_rig_meta.is_some();
    let base_triangle_stride = config.triangle_stride.max(1) as usize;
    let base_min_triangle_area_px2 = config.min_triangle_area_px2.max(0.0);
    let center_lock_restore_after_freefly = center_lock_enabled;
    let io_failure_count: u8 = 0;
    let ghostty_zoom_repaint_due: Option<Instant> = None;
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
    let mut osd_until: Option<Instant> = Some(Instant::now() + std::time::Duration::from_secs(2));
    let last_render_stats = RenderStats::default();
    let effective_aspect_state = resolve_cell_aspect(&config, detect_terminal_cell_aspect());
    let initial_orbit_camera = orbit_camera(
        orbit_state.angle,
        framing.radius.max(0.2),
        framing.camera_height,
        framing.focus,
    );
    let freefly_state = freefly_state_from_camera(initial_orbit_camera, freefly_speed);
    let initial_freefly_state = freefly_state;
    let loaded_camera_track = crate::runtime::audio_sync::load_camera_track(&camera_settings);
    let runtime_camera = RuntimeCameraState::new(
        wasd_mode,
        camera_settings.mode,
        loaded_camera_track.is_some(),
    );
    let color_recovery = ColorRecoveryState::from_requested(
        requested_color_mode,
        config.ansi_quantization,
        config.recover_color_auto,
    );
    let mut color_mode = requested_color_mode;
    let mut ansi_quantization = config.ansi_quantization;
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
        osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }
    if matches!(config.output_mode, RenderOutputMode::Hybrid) && active_graphics_protocol.is_none()
    {
        last_osd_notice = Some("hybrid fallback: text (graphics unsupported)".to_owned());
        osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
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
        osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }
    let render_cells = resize_runtime_frame(
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
        osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }
    let continuous_sync_state = ContinuousSyncState::default();
    let graphics_slow_streak: u32 = 0;
    let track_lost_streak: u32 = 0;
    let center_drift_streak: u32 = 0;
    if camera_settings.vmd_path.is_some() && loaded_camera_track.is_none() {
        eprintln!("warning: camera VMD could not be loaded. fallback to runtime camera.");
    }
    if scaled {
        osd_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }

    let start = Instant::now();
    let prev_wall_seconds = 0.0_f32;
    let frame_budget = if config.fps_cap == 0 {
        if matches!(
            config.output_mode,
            RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
        ) {
            let target = config.hq_target_fps.clamp(12, 120) as f32;
            Some(std::time::Duration::from_secs_f32(1.0 / target))
        } else {
            None
        }
    } else {
        Some(std::time::Duration::from_secs_f32(
            1.0 / (config.fps_cap as f32),
        ))
    };
    let fixed_step = 1.0 / 120.0_f32;
    let sim_time = 0.0_f32;
    let sim_accum = 0.0_f32;

    if let Some(audio) = audio_sync.as_ref() {
        audio.playback.sink.play();
    }

    Ok(BootstrapState {
        terminal,
        frame,
        pipeline,
        pmx_physics_state,
        glyph_ramp,
        render_scratch,
        display_cells,
        render_cells,
        active_graphics_protocol,
        framing: SceneFraming {
            radius: framing.radius,
            camera_height: framing.camera_height,
            focus: framing.focus,
            extent_y,
            has_stage_mesh,
        },
        orbit_state,
        model_spin_enabled,
        user_zoom,
        focus_offset,
        camera_height_offset,
        center_lock_enabled,
        center_lock_mode,
        stage_level,
        requested_color_mode,
        ascii_force_color_active,
        color_mode,
        ansi_quantization,
        braille_profile,
        cinematic_mode,
        camera_focus_mode,
        reactive_gain,
        exposure_bias,
        sync_offset_ms,
        contrast_preset,
        reactive_state,
        camera_director,
        adaptive_quality,
        visibility_watchdog,
        center_lock_state,
        auto_radius_guard,
        distance_clamp_guard,
        screen_fit,
        exposure_auto_boost,
        is_pmx_scene,
        base_triangle_stride,
        base_min_triangle_area_px2,
        center_lock_restore_after_freefly,
        initial_freefly_state,
        loaded_camera_track,
        runtime_camera,
        color_recovery,
        continuous_sync_state,
        graphics_slow_streak,
        track_lost_streak,
        center_drift_streak,
        resize_recovery_pending: false,
        sync_profile_dirty: false,
        last_osd_notice,
        osd_until,
        last_render_stats,
        effective_aspect_state,
        freefly_state,
        gpu_renderer_state,
        io_failure_count,
        ghostty_zoom_repaint_due,
        kitty_internal_res_base,
        frame_budget,
        fixed_step,
        sim_time,
        sim_accum,
        prev_wall_seconds,
        start,
        terminal_profile,
        sync_profile,
        animation_index,
        scene,
        config,
        audio_sync,
    })
}
