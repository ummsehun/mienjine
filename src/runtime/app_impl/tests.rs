use std::{fs, path::PathBuf};

use super::*;
use crate::{
    renderer::Camera,
    renderer::RenderStats,
    runtime::{
        asset_discovery::{self, discover_camera_vmds, discover_pmx_files, discover_vmd_files},
        audio_sync::{compute_animation_speed_factor, compute_animation_time},
        interaction::update_camera_director,
        options::{default_color_mode_for_mode, resolve_effective_color_mode},
        scene_analysis::compute_scene_framing,
        start_ui::{StageChoice, StageTransform},
        state::{
            apply_distant_subject_clarity_boost, apply_pmx_surface_guardrails, cap_render_size,
            dynamic_clip_planes, is_terminal_size_unstable, CameraDirectorState, CenterLockState,
            ContinuousSyncState, DistanceClampGuard, ExposureAutoBoost, OrbitState,
            RuntimeAdaptiveQuality, RuntimeCameraState, ScreenFitController,
            LOW_VIS_EXPOSURE_RECOVER_FRAMES, LOW_VIS_EXPOSURE_TRIGGER_FRAMES, MAX_RENDER_COLS,
            MAX_RENDER_ROWS,
        },
    },
    scene::{
        CameraControlMode, CameraFocusMode, CameraMode, CenterLockMode, CinematicCameraMode,
        ColorMode, PerfProfile, RenderMode, SyncPolicy, SyncSpeedMode,
    },
};
use glam::{Quat, Vec3};
use tempfile::tempdir;

#[path = "discovery_tests.rs"]
mod discovery_tests;

#[test]
fn auto_speed_factor_matches_reference_ratio() {
    let factor = compute_animation_speed_factor(
        Some(174.10),
        Some(170.480_907),
        SyncSpeedMode::AutoDurationFit,
    );
    assert!((factor - 1.021_229).abs() < 1e-4);
}

#[test]
fn auto_speed_factor_allows_large_duration_ratio() {
    let factor =
        compute_animation_speed_factor(Some(300.0), Some(120.0), SyncSpeedMode::AutoDurationFit);
    assert!((factor - 2.5).abs() < 1e-6);
}

#[test]
fn animation_time_applies_sync_offset_with_audio_clock() {
    let mut state = ContinuousSyncState::default();
    let time = compute_animation_time(
        &mut state,
        SyncPolicy::Fixed,
        0.016,
        5.0,
        Some(3.0),
        1.05,
        120,
        120,
        0.15,
        None,
    );
    assert!((time - 3.27).abs() < 1e-6);
}

#[test]
fn continuous_sync_tracks_drift_ema_and_hard_snaps() {
    let mut state = ContinuousSyncState::default();
    // First sample initializes near target.
    let _ = compute_animation_time(
        &mut state,
        SyncPolicy::Continuous,
        0.016,
        0.016,
        Some(0.0),
        1.0,
        0,
        120,
        0.15,
        None,
    );
    // Large target jump should trigger a hard snap and non-zero drift metric.
    let _ = compute_animation_time(
        &mut state,
        SyncPolicy::Continuous,
        0.016,
        0.032,
        Some(2.0),
        1.0,
        0,
        120,
        0.15,
        None,
    );
    assert!(state.drift_ema > 0.0);
    assert!(state.hard_snap_count >= 1);
}

fn simulate_continuous_sync(
    clip_duration: f32,
    audio_duration: f32,
    total_seconds: f32,
) -> (f32, u32, f32) {
    let dt = 1.0 / 60.0;
    let warmup = 10.0;
    let mut elapsed_wall = 0.0_f32;
    let mut max_err_after_warmup = 0.0_f32;
    let mut state = ContinuousSyncState::default();
    let speed_factor = compute_animation_speed_factor(
        Some(clip_duration),
        Some(audio_duration),
        SyncSpeedMode::AutoDurationFit,
    );

    while elapsed_wall < total_seconds {
        elapsed_wall += dt;
        let elapsed_audio = elapsed_wall.rem_euclid(audio_duration);
        let anim_time = compute_animation_time(
            &mut state,
            SyncPolicy::Continuous,
            dt,
            elapsed_wall,
            Some(elapsed_audio),
            speed_factor,
            0,
            120,
            0.15,
            Some(clip_duration),
        );
        let target = elapsed_audio * speed_factor;
        let raw = (target - anim_time).abs();
        let err = raw.min((clip_duration - raw).abs());
        if elapsed_wall >= warmup {
            max_err_after_warmup = max_err_after_warmup.max(err);
        }
    }

    (max_err_after_warmup, state.hard_snap_count, state.drift_ema)
}

#[test]
fn continuous_sync_converges_when_clip_longer_than_audio() {
    let (max_err, hard_snaps, drift_ema) = simulate_continuous_sync(120.0, 117.0, 180.0);
    assert!(max_err <= 0.120);
    assert!(hard_snaps <= 9);
    assert!(drift_ema.is_finite());
}

#[test]
fn continuous_sync_converges_when_audio_longer_than_clip() {
    let (max_err, hard_snaps, drift_ema) = simulate_continuous_sync(117.0, 120.0, 180.0);
    assert!(max_err <= 0.120);
    assert!(hard_snaps <= 9);
    assert!(drift_ema.is_finite());
}

#[test]
fn auto_framing_focus_y_uses_center() {
    let scene = crate::scene::cube_scene();
    let framing = compute_scene_framing(&scene, RenderConfig::default().fov_deg, 0.0, 0.0, 0.0);
    assert!(framing.focus.y.abs() < 0.05);
}

#[test]
fn mode_defaults_to_expected_color_mode() {
    assert!(matches!(
        default_color_mode_for_mode(RenderMode::Ascii),
        ColorMode::Mono
    ));
    assert!(matches!(
        default_color_mode_for_mode(RenderMode::Braille),
        ColorMode::Ansi
    ));
}

#[test]
fn ascii_force_color_overrides_requested_mono() {
    assert!(matches!(
        resolve_effective_color_mode(RenderMode::Ascii, ColorMode::Mono, true),
        ColorMode::Ansi
    ));
    assert!(matches!(
        resolve_effective_color_mode(RenderMode::Braille, ColorMode::Mono, true),
        ColorMode::Mono
    ));
}

#[test]
fn camera_mode_is_promoted_when_vmd_source_exists() {
    assert!(matches!(
        resolve_effective_camera_mode(CameraMode::Off, true),
        CameraMode::Vmd
    ));
    assert!(matches!(
        resolve_effective_camera_mode(CameraMode::Blend, true),
        CameraMode::Blend
    ));
    assert!(matches!(
        resolve_effective_camera_mode(CameraMode::Off, false),
        CameraMode::Off
    ));
}

#[test]
fn default_animation_prefers_non_morph_clip() {
    use crate::animation::{
        AnimationChannel, AnimationClip, ChannelTarget, ChannelValues, Interpolation,
    };
    use crate::scene::{MeshCpu, MeshInstance, MeshLayer, MorphTargetCpu, Node, SceneCpu};
    use glam::Vec3;

    let scene = SceneCpu {
        meshes: vec![MeshCpu {
            positions: vec![Vec3::ZERO],
            normals: vec![Vec3::Y],
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 0, 0]],
            joints4: None,
            weights4: None,
            morph_targets: vec![MorphTargetCpu {
                name: Some("move_up".to_owned()),
                position_deltas: vec![Vec3::new(0.0, 1.0, 0.0)],
                normal_deltas: vec![Vec3::ZERO],
            }],
        }],
        materials: Vec::new(),
        textures: Vec::new(),
        skins: Vec::new(),
        nodes: vec![Node {
            name: Some("root".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        }],
        mesh_instances: vec![MeshInstance {
            mesh_index: 0,
            node_index: 0,
            skin_index: None,
            default_morph_weights: vec![0.0],
            layer: MeshLayer::Subject,
        }],
        animations: vec![
            AnimationClip {
                name: Some("face".to_owned()),
                channels: vec![AnimationChannel {
                    node_index: 0,
                    target: ChannelTarget::MorphWeights,
                    interpolation: Interpolation::Linear,
                    inputs: vec![0.0, 1.0],
                    outputs: ChannelValues::MorphWeights {
                        values: vec![0.0, 1.0],
                        weights_per_key: 1,
                    },
                }],
                duration: 1.0,
                looping: true,
            },
            AnimationClip {
                name: Some("body".to_owned()),
                channels: vec![AnimationChannel {
                    node_index: 0,
                    target: ChannelTarget::Translation,
                    interpolation: Interpolation::Linear,
                    inputs: vec![0.0, 1.0],
                    outputs: ChannelValues::Vec3(vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)]),
                }],
                duration: 1.0,
                looping: true,
            },
        ],
        root_center_node: Some(0),
        pmx_rig_meta: None,
        material_morphs: Vec::new(),
    };

    assert_eq!(default_body_animation_index(&scene), Some(1));
}

#[test]
fn runtime_camera_starts_in_orbit_when_track_is_available() {
    let state = RuntimeCameraState::new(CameraControlMode::FreeFly, CameraMode::Vmd, true);
    assert!(matches!(state.control_mode, CameraControlMode::Orbit));
    assert!(state.track_enabled);
}

#[test]
fn distant_subject_clarity_boost_strengthens_subject_visibility() {
    let mut cfg = RenderConfig::default();
    cfg.model_lift = 0.10;
    cfg.edge_accent_strength = 0.20;
    cfg.bg_suppression = 0.20;
    cfg.triangle_stride = 3;
    cfg.min_triangle_area_px2 = 0.8;
    apply_distant_subject_clarity_boost(&mut cfg, 0.10);
    assert!(cfg.model_lift > 0.10);
    assert!(cfg.edge_accent_strength > 0.20);
    assert!(cfg.bg_suppression > 0.20);
    assert!(cfg.triangle_stride < 3);
    assert!(cfg.min_triangle_area_px2 < 0.8);
}

#[test]
fn pmx_surface_guardrails_clamp_sparse_rendering_on_small_subjects() {
    let mut cfg = RenderConfig::default();
    cfg.triangle_stride = 3;
    cfg.min_triangle_area_px2 = 0.8;
    cfg.edge_accent_strength = 0.9;

    apply_pmx_surface_guardrails(&mut cfg, true, 0.20);

    assert_eq!(cfg.triangle_stride, 1);
    assert!(cfg.min_triangle_area_px2 <= 0.12);
    assert!(cfg.edge_accent_strength <= 0.26);
}

#[test]
fn center_lock_camera_space_moves_camera_when_anchor_is_offcenter() {
    let mut state = CenterLockState::default();
    let mut stats = RenderStats::default();
    stats.subject_centroid_px = Some((10.0, 20.0));
    let mut camera = Camera::default();
    let before = camera.eye;
    state.apply_camera_space(
        &stats,
        CenterLockMode::Root,
        120,
        40,
        &mut camera,
        60.0,
        0.5,
        2.0,
    );
    assert!((camera.eye - before).length() > 1e-6);
}

#[test]
fn screen_fit_controller_uses_mode_specific_targets() {
    let mut controller = ScreenFitController::default();
    controller.update(0.40, RenderMode::Ascii, true);
    let ascii_gain = controller.auto_zoom_gain;
    assert!(ascii_gain > 1.0);

    controller = ScreenFitController::default();
    controller.update(0.40, RenderMode::Braille, true);
    let braille_gain = controller.auto_zoom_gain;
    assert!(braille_gain > 1.0);
    assert!(ascii_gain >= braille_gain);
}

#[test]
fn exposure_auto_boost_ramps_and_recovers() {
    let mut boost = ExposureAutoBoost::default();
    for _ in 0..LOW_VIS_EXPOSURE_TRIGGER_FRAMES {
        boost.update(0.001);
    }
    assert!(boost.boost > 0.0);
    let boosted = boost.boost;
    for _ in 0..LOW_VIS_EXPOSURE_RECOVER_FRAMES {
        boost.update(0.05);
    }
    assert!(boost.boost < boosted);
}

#[test]
fn camera_director_outputs_stable_values() {
    let mut director = CameraDirectorState::default();
    let (radius, height, focus_y, jitter) = update_camera_director(
        &mut director,
        CinematicCameraMode::On,
        CameraFocusMode::Auto,
        0.1,
        0.6,
        0.35,
        1.2,
        1.0,
    );
    assert!(radius > 0.0);
    assert!(height.abs() < 1.0);
    assert!(focus_y.abs() < 1.0);
    assert!(jitter.abs() <= 0.015 + 1e-3);
}

#[test]
fn orbit_state_holds_angle_when_disabled() {
    let mut orbit = OrbitState::new(0.0);
    orbit.angle = 1.23;
    orbit.advance(1.0);
    assert!((orbit.angle - 1.23).abs() < 1e-6);
}

#[test]
fn adaptive_quality_moves_lod_on_thresholds() {
    let mut quality = RuntimeAdaptiveQuality::new(PerfProfile::Balanced);
    for _ in 0..30 {
        quality.observe(90.0);
    }
    assert!(quality.lod_level >= 1);

    for _ in 0..90 {
        quality.observe(8.0);
    }
    assert!(quality.lod_level <= 1);
}

#[test]
fn cap_render_size_applies_upper_bound() {
    let (w, h, scaled) = cap_render_size(6000, 3200);
    assert!(scaled);
    assert!(w <= MAX_RENDER_COLS);
    assert!(h <= MAX_RENDER_ROWS);
}

#[test]
fn terminal_size_unstable_only_for_invalid_or_sentinel_values() {
    assert!(is_terminal_size_unstable(0, 40));
    assert!(is_terminal_size_unstable(120, 0));
    assert!(is_terminal_size_unstable(u16::MAX, 40));
    assert!(is_terminal_size_unstable(120, u16::MAX));
    assert!(!is_terminal_size_unstable(432, 102));
    assert!(!is_terminal_size_unstable(900, 140));
}

