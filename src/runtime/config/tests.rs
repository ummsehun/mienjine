//! Tests for runtime configuration parsing and defaults.

use std::path::Path;

use crate::runtime::config::load_gascii_config;
use crate::runtime::config::types::{GasciiConfig, UiLanguage};
use crate::runtime::config_parse::parse_bool;
use crate::runtime::sync_profile::SyncProfileMode;
use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, KittyCompression,
    KittyInternalResPreset, KittyTransport, PerfProfile, RenderBackend, RenderOutputMode,
    StageRole, SyncPolicy, SyncSpeedMode, TextureSamplerMode, TextureSamplingMode, TextureVOrigin,
    ThemeStyle,
};

#[test]
fn default_config_values() {
    let cfg = GasciiConfig::default();
    assert_eq!(cfg.ui_language, UiLanguage::Ko);
    assert_eq!(cfg.font_preset_steps, 0);
    assert!(!cfg.font_preset_enabled);
    assert_eq!(cfg.color_mode, None);
    assert!(cfg.ascii_force_color);
    assert_eq!(cfg.output_mode, RenderOutputMode::Text);
    assert_eq!(cfg.graphics_protocol, GraphicsProtocol::Auto);
    assert_eq!(cfg.kitty_transport, KittyTransport::Shm);
    assert_eq!(cfg.kitty_compression, KittyCompression::None);
    assert_eq!(cfg.kitty_internal_res, KittyInternalResPreset::R640x360);
    assert!((cfg.kitty_scale - 1.0).abs() < 1e-6);
    assert_eq!(cfg.hq_target_fps, 24);
    assert!(cfg.subject_exposure_only);
    assert_eq!(cfg.stage_role, StageRole::Sub);
    assert!((cfg.stage_luma_cap - 0.35).abs() < 1e-6);
    assert!(cfg.recover_color_auto);
    assert_eq!(cfg.braille_profile, BrailleProfile::Safe);
    assert_eq!(cfg.theme_style, ThemeStyle::Theater);
    assert_eq!(cfg.audio_reactive, AudioReactiveMode::On);
    assert_eq!(cfg.cinematic_camera, CinematicCameraMode::On);
    assert!((cfg.reactive_gain - 0.35).abs() < 1e-6);
    assert_eq!(cfg.perf_profile, PerfProfile::Balanced);
    assert_eq!(cfg.detail_profile, DetailProfile::Balanced);
    assert_eq!(cfg.clarity_profile, ClarityProfile::Sharp);
    assert_eq!(cfg.ansi_quantization, AnsiQuantization::Q216);
    assert_eq!(cfg.backend, RenderBackend::Cpu);
    assert_eq!(cfg.stage_dir, std::path::PathBuf::from("assets/stage"));
    assert_eq!(cfg.stage_selection, "auto");
    assert!((cfg.exposure_bias - 0.0).abs() < 1e-6);
    assert!(cfg.center_lock);
    assert_eq!(cfg.center_lock_mode, CenterLockMode::Root);
    assert_eq!(cfg.wasd_mode, CameraControlMode::FreeFly);
    assert!((cfg.freefly_speed - 1.0).abs() < 1e-6);
    assert!((cfg.camera_look_speed - 1.0).abs() < 1e-6);
    assert_eq!(cfg.camera_dir, std::path::PathBuf::from("assets/camera"));
    assert_eq!(cfg.camera_selection, "none");
    assert_eq!(cfg.camera_mode, CameraMode::Off);
    assert_eq!(cfg.camera_align_preset, CameraAlignPreset::Std);
    assert!((cfg.camera_unit_scale - 0.08).abs() < 1e-6);
    assert!((cfg.camera_vmd_fps - 30.0).abs() < 1e-6);
    assert_eq!(cfg.camera_vmd_path, None);
    assert_eq!(cfg.camera_focus, CameraFocusMode::Auto);
    assert!(cfg.material_color);
    assert_eq!(cfg.texture_sampling, TextureSamplingMode::Nearest);
    assert_eq!(cfg.texture_v_origin, TextureVOrigin::Gltf);
    assert_eq!(cfg.texture_sampler, TextureSamplerMode::Gltf);
    assert!((cfg.braille_aspect_compensation - 1.00).abs() < 1e-6);
    assert!((cfg.model_lift - 0.12).abs() < 1e-6);
    assert!((cfg.edge_accent_strength - 0.32).abs() < 1e-6);
    assert!((cfg.bg_suppression - 0.35).abs() < 1e-6);
    assert_eq!(cfg.stage_level, 2);
    assert!(cfg.stage_reactive);
    assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Auto);
    assert_eq!(cfg.cell_aspect_trim, 1.0);
    assert_eq!(cfg.contrast_profile, ContrastProfile::Adaptive);
    assert_eq!(cfg.sync_offset_ms, 0);
    assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::AutoDurationFit);
    assert_eq!(cfg.sync_policy, SyncPolicy::Continuous);
    assert_eq!(cfg.sync_hard_snap_ms, 120);
    assert!((cfg.sync_kp - 0.15).abs() < 1e-6);
    assert_eq!(
        cfg.sync_profile_dir,
        std::path::PathBuf::from("assets/sync")
    );
    assert_eq!(cfg.sync_profile_mode, SyncProfileMode::Auto);
    assert_eq!(cfg.upscale_factor, 2);
    assert!((cfg.upscale_sharpen - 0.20).abs() < 1e-6);
    assert_eq!(cfg.triangle_stride, 1);
    assert_eq!(cfg.min_triangle_area_px2, 0.0);
}

#[test]
fn parse_bool_variants() {
    assert_eq!(parse_bool("true"), Some(true));
    assert_eq!(parse_bool("off"), Some(false));
    assert_eq!(parse_bool("??"), None);
}

#[test]
fn legacy_font_keys_remain_compatible() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("Gascii.config");
    std::fs::write(
        &path,
        "ghostty_font_reset = true\nghostty_font_steps = 3\nui_language = en\n",
    )
    .expect("write config");

    let cfg = load_gascii_config(&path);
    assert_eq!(cfg.ui_language, UiLanguage::En);
    assert!(cfg.font_preset_enabled);
    assert_eq!(cfg.font_preset_steps, 3);
}

#[test]
fn normalized_font_keys_parse_correctly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("Gascii.config");
    std::fs::write(
        &path,
        "font_preset_enabled = true\nfont_preset_steps = -2\ntriangle_stride = 4\n",
    )
    .expect("write config");

    let cfg = load_gascii_config(&path);
    assert!(cfg.font_preset_enabled);
    assert_eq!(cfg.font_preset_steps, -2);
    assert_eq!(cfg.triangle_stride, 4);
}

#[test]
fn parse_new_visual_and_sync_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("Gascii.config");
    std::fs::write(
        &path,
        "cell_aspect_mode = manual\ncell_aspect_trim = 1.15\ncontrast_profile = fixed\n\
         sync_offset_ms = -120\nsync_speed_mode = realtime\nsync_policy = fixed\n\
         sync_hard_snap_ms = 160\nsync_kp = 0.2\nsync_profile_dir = assets/sync/custom\n\
         sync_profile_mode = write\ncolor_mode=ansi\nascii_force_color=false\n\
         output_mode=kitty-hq\nkitty_transport=direct\nkitty_compression=zlib\n\
         kitty_internal_res=1280x720\nkitty_scale=1.25\nhq_target_fps=30\n\
         subject_exposure_only=off\nstage_role=off\nstage_luma_cap=0.55\n\
         recover_color=off\ngraphics_protocol=kitty\nupscale_factor=4\nupscale_sharpen=0.6\n\
         braille_profile=normal\ntheme=holo\naudio_reactive=high\n\
         cinematic_camera=aggressive\nreactive_gain=0.42\nperf_profile=smooth\n\
         detail_profile=ultra\nclarity_profile=extreme\nansi_quantization=off\n\
         backend=gpu-preview\nstage_dir=assets/stage\nstage_selection=world is mine\n\
         exposure_bias=0.18\ncenter_lock=false\ncenter_lock_mode=mixed\n\
         wasd_mode=orbit\nfreefly_speed=2.4\ncamera_look_speed=1.8\n\
         camera_dir=assets/camera\ncamera_selection=none\ncamera_mode=blend\n\
         camera_align_preset=alt-b\ncamera_unit_scale=0.12\ncamera_vmd_fps=60\n\
         camera_vmd_path=assets/camera/world_is_mine.vmd\ncamera_focus=face\n\
         material_color=off\ntexture_sampling=bilinear\ntexture_v_origin=legacy\n\
         texture_sampler=override\nbraille_aspect_compensation=1.12\n\
         model_lift=0.2\nedge_accent_strength=0.5\nbg_suppression=0.42\n\
         stage_level=4\nstage_reactive=off\n",
    )
    .expect("write config");

    let cfg = load_gascii_config(&path);
    assert_eq!(cfg.color_mode, Some(ColorMode::Ansi));
    assert!(!cfg.ascii_force_color);
    assert_eq!(cfg.output_mode, RenderOutputMode::KittyHq);
    assert_eq!(cfg.kitty_transport, KittyTransport::Direct);
    assert_eq!(cfg.kitty_compression, KittyCompression::Zlib);
    assert_eq!(cfg.kitty_internal_res, KittyInternalResPreset::R1280x720);
    assert!((cfg.kitty_scale - 1.25).abs() < 1e-6);
    assert_eq!(cfg.hq_target_fps, 30);
    assert!(!cfg.subject_exposure_only);
    assert_eq!(cfg.stage_role, StageRole::Off);
    assert!((cfg.stage_luma_cap - 0.55).abs() < 1e-6);
    assert!(!cfg.recover_color_auto);
    assert_eq!(cfg.graphics_protocol, GraphicsProtocol::Kitty);
    assert_eq!(cfg.upscale_factor, 4);
    assert!((cfg.upscale_sharpen - 0.6).abs() < 1e-6);
    assert_eq!(cfg.braille_profile, BrailleProfile::Normal);
    assert_eq!(cfg.theme_style, ThemeStyle::Holo);
    assert_eq!(cfg.audio_reactive, AudioReactiveMode::High);
    assert_eq!(cfg.cinematic_camera, CinematicCameraMode::Aggressive);
    assert!((cfg.reactive_gain - 0.42).abs() < 1e-6);
    assert_eq!(cfg.perf_profile, PerfProfile::Smooth);
    assert_eq!(cfg.detail_profile, DetailProfile::Ultra);
    assert_eq!(cfg.clarity_profile, ClarityProfile::Extreme);
    assert_eq!(cfg.ansi_quantization, AnsiQuantization::Off);
    assert_eq!(cfg.backend, RenderBackend::Gpu);
    assert_eq!(cfg.stage_dir, std::path::PathBuf::from("assets/stage"));
    assert_eq!(cfg.stage_selection, "world is mine");
    assert!((cfg.exposure_bias - 0.18).abs() < 1e-6);
    assert!(!cfg.center_lock);
    assert_eq!(cfg.center_lock_mode, CenterLockMode::Mixed);
    assert_eq!(cfg.wasd_mode, CameraControlMode::Orbit);
    assert!((cfg.freefly_speed - 2.4).abs() < 1e-6);
    assert!((cfg.camera_look_speed - 1.8).abs() < 1e-6);
    assert_eq!(cfg.camera_dir, std::path::PathBuf::from("assets/camera"));
    assert_eq!(cfg.camera_selection, "none");
    assert_eq!(cfg.camera_mode, CameraMode::Blend);
    assert_eq!(cfg.camera_align_preset, CameraAlignPreset::AltB);
    assert!((cfg.camera_unit_scale - 0.12).abs() < 1e-6);
    assert!((cfg.camera_vmd_fps - 60.0).abs() < 1e-6);
    assert_eq!(
        cfg.camera_vmd_path.as_deref(),
        Some(Path::new("assets/camera/world_is_mine.vmd"))
    );
    assert_eq!(cfg.camera_focus, CameraFocusMode::Face);
    assert!(!cfg.material_color);
    assert_eq!(cfg.texture_sampling, TextureSamplingMode::Bilinear);
    assert_eq!(cfg.texture_v_origin, TextureVOrigin::Legacy);
    assert_eq!(cfg.texture_sampler, TextureSamplerMode::Override);
    assert!((cfg.braille_aspect_compensation - 1.12).abs() < 1e-6);
    assert!((cfg.model_lift - 0.2).abs() < 1e-6);
    assert!((cfg.edge_accent_strength - 0.5).abs() < 1e-6);
    assert!((cfg.bg_suppression - 0.42).abs() < 1e-6);
    assert_eq!(cfg.stage_level, 4);
    assert!(!cfg.stage_reactive);
    assert_eq!(cfg.cell_aspect_mode, CellAspectMode::Manual);
    assert_eq!(cfg.cell_aspect_trim, 1.15);
    assert_eq!(cfg.contrast_profile, ContrastProfile::Fixed);
    assert_eq!(cfg.sync_offset_ms, -120);
    assert_eq!(cfg.sync_speed_mode, SyncSpeedMode::Realtime1x);
    assert_eq!(cfg.sync_policy, SyncPolicy::Fixed);
    assert_eq!(cfg.sync_hard_snap_ms, 160);
    assert!((cfg.sync_kp - 0.2).abs() < 1e-6);
    assert_eq!(
        cfg.sync_profile_dir,
        std::path::PathBuf::from("assets/sync/custom")
    );
    assert_eq!(cfg.sync_profile_mode, SyncProfileMode::Write);
}
