use std::path::Path;

use anyhow::Result;

use crate::{
    runtime::{
        config::{load_gascii_config, GasciiConfig},
        options::RuntimeSyncProfileContext,
        sync_profile::{SyncProfileEntry, SyncProfileStore},
    },
    scene::RenderConfig,
    shared::constants::SYNC_OFFSET_LIMIT_MS,
};

pub(crate) fn load_runtime_config() -> GasciiConfig {
    load_gascii_config(Path::new("Gascii.config"))
}

pub(crate) fn apply_runtime_render_tuning(config: &mut RenderConfig, runtime_cfg: &GasciiConfig) {
    config.triangle_stride = runtime_cfg.triangle_stride.max(1);
    config.min_triangle_area_px2 = runtime_cfg.min_triangle_area_px2.max(0.0);
    config.braille_aspect_compensation = runtime_cfg.braille_aspect_compensation;
}

pub(crate) fn persist_sync_profile_offset(
    context: &RuntimeSyncProfileContext,
    sync_offset_ms: i32,
) -> Result<()> {
    let mut store = SyncProfileStore::load(&context.store_path)?;
    let mut merged = SyncProfileEntry::with_offset(
        sync_offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
    );
    if let Some(existing) = store.get(&context.key) {
        merged.sync_hard_snap_ms = existing.sync_hard_snap_ms;
        merged.sync_kp = existing.sync_kp;
        merged.sync_speed_mode = existing.sync_speed_mode;
    }
    store.upsert(context.key.clone(), merged);
    store.save_atomic(&context.store_path)
}
