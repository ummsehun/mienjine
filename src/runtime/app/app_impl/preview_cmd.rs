use anyhow::Result;

use crate::{
    cli::PreviewArgs,
    interfaces::preview::run_preview_server,
    runtime::{
        asset_discovery::{discover_camera_vmds, resolve_camera_vmd_choice},
        options::RuntimeSyncProfileContext,
        sync_profile::{
            SyncProfileMode, SyncProfileStore, build_profile_key, default_profile_store_path,
        },
    },
};

use super::config::load_runtime_config;

pub(crate) fn preview(args: PreviewArgs) -> Result<()> {
    let runtime_cfg = load_runtime_config();
    let camera_dir = runtime_cfg.camera_dir.clone();
    let camera_files = discover_camera_vmds(&camera_dir);
    let selector_explicit_none = runtime_cfg.camera_selection.eq_ignore_ascii_case("none");
    let camera_path = args
        .camera_vmd
        .clone()
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                runtime_cfg.camera_vmd_path.clone()
            }
        })
        .or_else(|| {
            if selector_explicit_none {
                None
            } else {
                resolve_camera_vmd_choice(&camera_dir, &camera_files, &runtime_cfg.camera_selection)
            }
        });
    let profile_key = build_profile_key(
        "glb",
        Some(args.glb.as_path()),
        None,
        camera_path.as_deref(),
    );
    let (profile_hit, resolved_offset) =
        if matches!(runtime_cfg.sync_profile_mode, SyncProfileMode::Off) {
            (false, runtime_cfg.sync_offset_ms)
        } else {
            let store_path = default_profile_store_path(&runtime_cfg.sync_profile_dir);
            match SyncProfileStore::load(&store_path) {
                Ok(store) => match store.get(&profile_key) {
                    Some(entry) => (true, entry.sync_offset_ms),
                    None => (false, runtime_cfg.sync_offset_ms),
                },
                Err(err) => {
                    eprintln!(
                        "warning: preview sync profile load failed {}: {err}",
                        store_path.display()
                    );
                    (false, runtime_cfg.sync_offset_ms)
                }
            }
        };
    run_preview_server(
        &args,
        camera_path,
        resolved_offset,
        if matches!(runtime_cfg.sync_profile_mode, SyncProfileMode::Off) {
            None
        } else {
            Some(profile_key)
        },
        profile_hit,
    )
}

#[allow(dead_code)]
fn _type_anchor(_: Option<RuntimeSyncProfileContext>) {}
