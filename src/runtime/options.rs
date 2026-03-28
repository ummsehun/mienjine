use std::path::{Path, PathBuf};

pub(crate) use crate::runtime::options_visual::{
    resolve_visual_options_for_bench, resolve_visual_options_for_run,
    resolve_visual_options_for_start,
};

use crate::{
    cli::{RunArgs, RunSceneArg, StartArgs},
    runtime::{
        config::GasciiConfig,
        sync_profile::{
            build_profile_key, default_profile_store_path, SyncProfileEntry, SyncProfileMode,
            SyncProfileStore,
        },
    },
    scene::{
        AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
        CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
        ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol,
        KittyCompression, KittyInternalResPreset, KittyPipelineMode, KittyTransport, PerfProfile,
        RecoverStrategy, RenderBackend, RenderMode, RenderOutputMode, StageRole, SyncPolicy,
        SyncSpeedMode, TextureSamplingMode, ThemeStyle,
    },
};

const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedVisualOptions {
    pub(crate) output_mode: RenderOutputMode,
    pub(crate) recover_color_auto: bool,
    pub(crate) graphics_protocol: GraphicsProtocol,
    pub(crate) kitty_transport: KittyTransport,
    pub(crate) kitty_compression: KittyCompression,
    pub(crate) kitty_internal_res: KittyInternalResPreset,
    pub(crate) kitty_pipeline_mode: KittyPipelineMode,
    pub(crate) recover_strategy: RecoverStrategy,
    pub(crate) kitty_scale: f32,
    pub(crate) hq_target_fps: u32,
    pub(crate) subject_exposure_only: bool,
    pub(crate) subject_target_height_ratio: f32,
    pub(crate) subject_target_width_ratio: f32,
    pub(crate) quality_auto_distance: bool,
    pub(crate) texture_mip_bias: f32,
    pub(crate) stage_as_sub_only: bool,
    pub(crate) stage_role: StageRole,
    pub(crate) stage_luma_cap: f32,
    pub(crate) cell_aspect_mode: CellAspectMode,
    pub(crate) cell_aspect_trim: f32,
    pub(crate) contrast_profile: ContrastProfile,
    pub(crate) perf_profile: PerfProfile,
    pub(crate) detail_profile: DetailProfile,
    pub(crate) backend: RenderBackend,
    pub(crate) exposure_bias: f32,
    pub(crate) center_lock: bool,
    pub(crate) center_lock_mode: CenterLockMode,
    pub(crate) wasd_mode: CameraControlMode,
    pub(crate) freefly_speed: f32,
    pub(crate) camera_look_speed: f32,
    pub(crate) camera_mode: CameraMode,
    pub(crate) camera_align_preset: CameraAlignPreset,
    pub(crate) camera_unit_scale: f32,
    pub(crate) camera_vmd_fps: f32,
    pub(crate) camera_vmd_path: Option<PathBuf>,
    pub(crate) camera_focus: CameraFocusMode,
    pub(crate) material_color: bool,
    pub(crate) texture_sampling: TextureSamplingMode,
    pub(crate) texture_v_origin: crate::scene::TextureVOrigin,
    pub(crate) texture_sampler: crate::scene::TextureSamplerMode,
    pub(crate) clarity_profile: ClarityProfile,
    pub(crate) ansi_quantization: AnsiQuantization,
    pub(crate) model_lift: f32,
    pub(crate) edge_accent_strength: f32,
    pub(crate) bg_suppression: f32,
    pub(crate) braille_aspect_compensation: f32,
    pub(crate) stage_level: u8,
    pub(crate) stage_reactive: bool,
    pub(crate) color_mode: Option<ColorMode>,
    pub(crate) ascii_force_color: bool,
    pub(crate) braille_profile: BrailleProfile,
    pub(crate) theme_style: ThemeStyle,
    pub(crate) audio_reactive: AudioReactiveMode,
    pub(crate) cinematic_camera: CinematicCameraMode,
    pub(crate) reactive_gain: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedSyncOptions {
    pub(crate) sync_offset_ms: i32,
    pub(crate) sync_speed_mode: SyncSpeedMode,
    pub(crate) sync_policy: SyncPolicy,
    pub(crate) sync_hard_snap_ms: u32,
    pub(crate) sync_kp: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSyncProfileOptions {
    pub(crate) mode: SyncProfileMode,
    pub(crate) profile_dir: PathBuf,
    pub(crate) key_override: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSyncProfileContext {
    pub(crate) mode: SyncProfileMode,
    pub(crate) store_path: PathBuf,
    pub(crate) key: String,
    pub(crate) hit: bool,
}

fn resolve_sync_options_common(
    sync_offset_ms: Option<i32>,
    sync_speed_mode: Option<SyncSpeedMode>,
    sync_policy: Option<SyncPolicy>,
    sync_hard_snap_ms: Option<u32>,
    sync_kp: Option<f32>,
    runtime_cfg: &GasciiConfig,
    profile: Option<&SyncProfileEntry>,
) -> ResolvedSyncOptions {
    let profile_speed_mode = profile.and_then(|entry| entry.sync_speed_mode);
    let profile_hard_snap = profile.and_then(|entry| entry.sync_hard_snap_ms);
    let profile_kp = profile.and_then(|entry| entry.sync_kp);
    let profile_offset = profile.map(|entry| entry.sync_offset_ms);
    ResolvedSyncOptions {
        sync_offset_ms: sync_offset_ms
            .or(profile_offset)
            .unwrap_or(runtime_cfg.sync_offset_ms)
            .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
        sync_speed_mode: sync_speed_mode
            .or(profile_speed_mode)
            .unwrap_or(runtime_cfg.sync_speed_mode),
        sync_policy: sync_policy.unwrap_or(runtime_cfg.sync_policy),
        sync_hard_snap_ms: sync_hard_snap_ms
            .or(profile_hard_snap)
            .unwrap_or(runtime_cfg.sync_hard_snap_ms)
            .clamp(10, 2_000),
        sync_kp: sync_kp
            .unwrap_or(profile_kp.unwrap_or(runtime_cfg.sync_kp))
            .clamp(0.01, 1.0),
    }
}

fn resolve_sync_profile_options_common(
    mode: SyncProfileMode,
    profile_dir: Option<PathBuf>,
    key_override: Option<String>,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncProfileOptions {
    ResolvedSyncProfileOptions {
        mode,
        profile_dir: profile_dir.unwrap_or_else(|| runtime_cfg.sync_profile_dir.clone()),
        key_override: key_override.filter(|value| !value.is_empty()),
    }
}

pub(crate) fn resolve_sync_options_for_start(
    args: &StartArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncOptions {
    resolve_sync_options_common(
        args.sync_offset_ms,
        args.sync_speed_mode.map(Into::into),
        args.sync_policy.map(Into::into),
        args.sync_hard_snap_ms,
        args.sync_kp,
        runtime_cfg,
        None,
    )
}

pub(crate) fn resolve_sync_options_for_run(
    args: &RunArgs,
    runtime_cfg: &GasciiConfig,
    profile: Option<&SyncProfileEntry>,
) -> ResolvedSyncOptions {
    resolve_sync_options_common(
        args.sync_offset_ms,
        args.sync_speed_mode.map(Into::into),
        args.sync_policy.map(Into::into),
        args.sync_hard_snap_ms,
        args.sync_kp,
        runtime_cfg,
        profile,
    )
}

pub(crate) fn resolve_sync_profile_options_for_start(
    args: &StartArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncProfileOptions {
    resolve_sync_profile_options_common(
        args.sync_profile_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_profile_mode),
        args.sync_profile_dir.clone(),
        args.sync_profile_key.clone(),
        runtime_cfg,
    )
}

pub(crate) fn resolve_sync_profile_options_for_run(
    args: &RunArgs,
    runtime_cfg: &GasciiConfig,
) -> ResolvedSyncProfileOptions {
    resolve_sync_profile_options_common(
        args.sync_profile_mode
            .map(Into::into)
            .unwrap_or(runtime_cfg.sync_profile_mode),
        args.sync_profile_dir.clone(),
        args.sync_profile_key.clone(),
        runtime_cfg,
    )
}

pub(crate) fn resolve_sync_profile_for_assets(
    options: &ResolvedSyncProfileOptions,
    scene_kind: RunSceneArg,
    glb_path: Option<&Path>,
    music_path: Option<&Path>,
    camera_path: Option<&Path>,
) -> (Option<RuntimeSyncProfileContext>, Option<SyncProfileEntry>) {
    if matches!(options.mode, SyncProfileMode::Off) {
        return (None, None);
    }
    let scene_kind = match scene_kind {
        RunSceneArg::Cube => "cube",
        RunSceneArg::Obj => "obj",
        RunSceneArg::Glb => "glb",
        RunSceneArg::Pmx => "pmx",
    };
    let key = options
        .key_override
        .clone()
        .unwrap_or_else(|| build_profile_key(scene_kind, glb_path, music_path, camera_path));
    let store_path = default_profile_store_path(&options.profile_dir);
    let profile = match SyncProfileStore::load(&store_path) {
        Ok(store) => store.get(&key).cloned(),
        Err(err) => {
            eprintln!(
                "warning: failed to load sync profiles {}: {err}",
                store_path.display()
            );
            None
        }
    };
    (
        Some(RuntimeSyncProfileContext {
            mode: options.mode,
            store_path,
            key,
            hit: profile.is_some(),
        }),
        profile,
    )
}

pub(crate) fn default_color_mode_for_mode(mode: RenderMode) -> ColorMode {
    match mode {
        RenderMode::Braille => ColorMode::Ansi,
        RenderMode::Ascii => ColorMode::Mono,
    }
}

pub(crate) fn resolve_effective_color_mode(
    mode: RenderMode,
    requested: ColorMode,
    ascii_force_color: bool,
) -> ColorMode {
    if matches!(mode, RenderMode::Ascii) && ascii_force_color {
        ColorMode::Ansi
    } else {
        requested
    }
}

pub(crate) fn resolve_effective_camera_mode(mode: CameraMode, has_vmd_source: bool) -> CameraMode {
    if has_vmd_source && matches!(mode, CameraMode::Off) {
        CameraMode::Vmd
    } else {
        mode
    }
}

pub(crate) fn color_path_label(
    color_mode: ColorMode,
    quantization: AnsiQuantization,
) -> &'static str {
    match color_mode {
        ColorMode::Mono => "mono",
        ColorMode::Ansi => match quantization {
            AnsiQuantization::Q216 => "ansi-q216",
            AnsiQuantization::Off => "ansi-truecolor",
        },
    }
}
