pub(crate) use crate::shared::constants::{SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_STEP_MS};

mod backend;
mod models;
mod pmx_physics;
mod quality;
mod quality_tuning;
mod sizing;
mod status;

pub(crate) use backend::{normalize_graphics_settings, resolve_runtime_backend};
pub(crate) use models::{
    CameraDirectorState, CameraShot, ColorRecoveryState, ContinuousSyncState, OrbitState,
    ReactiveState, RuntimeCameraSettings, RuntimeCameraState, RuntimeContrastPreset,
    RuntimeInputResult, RuntimePmxSettings,
};
pub(crate) use pmx_physics::{derive_pmx_profile, PmxPhysicsState};
pub(crate) use quality::{
    apply_runtime_contrast_preset, dynamic_clip_planes, AutoRadiusGuard, CenterLockState,
    DistanceClampGuard, ExposureAutoBoost, RuntimeAdaptiveQuality, ScreenFitController,
    VisibilityWatchdog,
};
#[allow(unused_imports)]
pub(crate) use quality::{
    LOW_VIS_EXPOSURE_RECOVER_FRAMES, LOW_VIS_EXPOSURE_RECOVER_THRESHOLD,
    LOW_VIS_EXPOSURE_THRESHOLD, LOW_VIS_EXPOSURE_TRIGGER_FRAMES, MIN_VISIBLE_HEIGHT_RATIO,
    MIN_VISIBLE_HEIGHT_RECOVER_FRAMES, MIN_VISIBLE_HEIGHT_RECOVER_RATIO,
    MIN_VISIBLE_HEIGHT_TRIGGER_FRAMES, VISIBILITY_LOW_FRAMES_TO_RECOVER, VISIBILITY_LOW_THRESHOLD,
};
pub(crate) use quality_tuning::{
    apply_adaptive_quality_tuning, apply_distant_subject_clarity_boost,
    apply_face_focus_detail_boost, apply_pmx_surface_guardrails, jitter_scale_for_lod,
};
pub(crate) use sizing::{cap_render_size, is_terminal_size_unstable};
#[allow(unused_imports)]
pub(crate) use sizing::{MAX_RENDER_COLS, MAX_RENDER_ROWS};
pub(crate) use status::{format_runtime_status, overlay_osd};
pub(crate) const HYBRID_GRAPHICS_MAX_CELLS: usize = 24_000;
pub(crate) const HYBRID_GRAPHICS_SLOW_FRAME_MS: f32 = 45.0;
pub(crate) const HYBRID_GRAPHICS_SLOW_STREAK_LIMIT: u32 = 8;
