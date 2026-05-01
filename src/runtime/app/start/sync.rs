use crate::{
    cli::{RunSceneArg, StartArgs},
    interfaces::tui::start_ui::{ModelBranch, StartSelection},
    runtime::{
        audio_sync::prepare_audio_sync,
        options::{
            ResolvedSyncOptions, ResolvedSyncProfileOptions, RuntimeSyncProfileContext,
            resolve_sync_profile_for_assets,
        },
        sync_profile::SyncProfileEntry,
    },
    scene::SceneCpu,
};

pub(super) struct SyncResult {
    pub effective_sync: ResolvedSyncOptions,
    pub audio_sync: Option<crate::runtime::audio_sync::AudioSyncRuntime>,
    pub sync_profile_context: Option<RuntimeSyncProfileContext>,
}

pub(super) fn resolve_sync_and_audio(
    selection: &StartSelection,
    scene: &SceneCpu,
    animation_index: Option<usize>,
    sync_profile_opts: &ResolvedSyncProfileOptions,
    sync_defaults: &ResolvedSyncOptions,
    args: &StartArgs,
) -> SyncResult {
    let (sync_profile_context, sync_profile_entry) = resolve_sync_profile_for_assets(
        sync_profile_opts,
        match selection.branch {
            ModelBranch::Glb => RunSceneArg::Glb,
            ModelBranch::PmxVmd => RunSceneArg::Pmx,
        },
        Some(match selection.branch {
            ModelBranch::Glb => selection.glb_path.as_path(),
            ModelBranch::PmxVmd => selection
                .pmx_path
                .as_deref()
                .unwrap_or(selection.glb_path.as_path()),
        }),
        selection.music_path.as_deref(),
        selection.camera_vmd_path.as_deref(),
    );

    let mut effective_sync = ResolvedSyncOptions {
        sync_offset_ms: selection.sync_offset_ms,
        sync_speed_mode: selection.sync_speed_mode,
        sync_policy: selection.sync_policy,
        sync_hard_snap_ms: selection.sync_hard_snap_ms,
        sync_kp: selection.sync_kp,
    };

    if let Some(profile) = sync_profile_entry.as_ref() {
        apply_profile_overrides(&mut effective_sync, profile, sync_defaults, args);
    }

    let clip_duration_secs = animation_index
        .and_then(|idx| scene.animations.get(idx))
        .map(|clip| clip.duration);

    let audio_sync = prepare_audio_sync(
        selection.music_path.as_deref(),
        clip_duration_secs,
        effective_sync.sync_speed_mode,
    );

    if selection.music_path.is_some() && audio_sync.is_none() {
        eprintln!("warning: audio playback unavailable. continuing in silent mode.");
    }

    SyncResult {
        effective_sync,
        audio_sync,
        sync_profile_context,
    }
}

fn apply_profile_overrides(
    effective: &mut ResolvedSyncOptions,
    profile: &SyncProfileEntry,
    defaults: &ResolvedSyncOptions,
    args: &StartArgs,
) {
    if args.sync_offset_ms.is_none() && effective.sync_offset_ms == defaults.sync_offset_ms {
        effective.sync_offset_ms = profile.sync_offset_ms;
    }
    if args.sync_speed_mode.is_none()
        && effective.sync_speed_mode == defaults.sync_speed_mode
        && profile.sync_speed_mode.is_some()
    {
        effective.sync_speed_mode = profile.sync_speed_mode.unwrap_or(defaults.sync_speed_mode);
    }
    if args.sync_hard_snap_ms.is_none()
        && effective.sync_hard_snap_ms == defaults.sync_hard_snap_ms
        && profile.sync_hard_snap_ms.is_some()
    {
        effective.sync_hard_snap_ms = profile
            .sync_hard_snap_ms
            .unwrap_or(defaults.sync_hard_snap_ms)
            .clamp(10, 2_000);
    }
    if args.sync_kp.is_none() && effective.sync_kp == defaults.sync_kp && profile.sync_kp.is_some()
    {
        effective.sync_kp = profile.sync_kp.unwrap_or(defaults.sync_kp).clamp(0.01, 1.0);
    }
}
