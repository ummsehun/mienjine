//! Parser for sync settings: offset, speed, policy, profile, upscale, geometry.

use std::path::PathBuf;

use crate::runtime::config::types::GasciiConfig;
use crate::runtime::sync_profile::SyncProfileMode;
use crate::scene::{SyncPolicy, SyncSpeedMode};
use crate::shared::constants::SYNC_OFFSET_LIMIT_MS;

/// Parse `sync_offset_ms`.
pub fn parse_sync_offset_ms(value: &str) -> Option<i32> {
    value
        .parse::<i32>()
        .ok()
        .map(|v| v.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS))
}

/// Parse `sync_speed_mode`.
pub fn parse_sync_speed_mode(value: &str) -> SyncSpeedMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("real") || lower == "1x" {
        SyncSpeedMode::Realtime1x
    } else {
        SyncSpeedMode::AutoDurationFit
    }
}

/// Parse `sync_policy`.
pub fn parse_sync_policy(value: &str) -> SyncPolicy {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("fix") {
        SyncPolicy::Fixed
    } else if lower.starts_with("man") {
        SyncPolicy::Manual
    } else {
        SyncPolicy::Continuous
    }
}

/// Parse `sync_hard_snap_ms`.
pub fn parse_sync_hard_snap_ms(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().map(|v| v.clamp(10, 2000))
}

/// Parse `sync_kp`.
pub fn parse_sync_kp(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.01, 1.0))
}

/// Parse `sync_profile_dir`.
pub fn parse_sync_profile_dir(value: &str) -> Option<PathBuf> {
    let raw = value.trim().trim_matches('"').trim_matches('\'');
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

/// Parse `sync_profile_mode`.
pub fn parse_sync_profile_mode(value: &str) -> SyncProfileMode {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("off") || lower == "0" {
        SyncProfileMode::Off
    } else if lower.starts_with("wri") {
        SyncProfileMode::Write
    } else {
        SyncProfileMode::Auto
    }
}

/// Parse `upscale_factor`.
pub fn parse_upscale_factor(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().map(|v| match v {
        1 | 2 | 4 => v,
        _ => 2,
    })
}

/// Parse `upscale_sharpen`.
pub fn parse_upscale_sharpen(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 2.0))
}

/// Parse `triangle_stride`, `tri_stride`.
pub fn parse_triangle_stride(value: &str) -> Option<usize> {
    value.parse::<usize>().ok().map(|v| v.clamp(1, 16))
}

/// Parse `min_triangle_area_px2`, `tiny_triangle_area_px2`.
pub fn parse_min_triangle_area_px2(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 16.0))
}

/// Apply sync keys to config.
pub fn apply_sync(key: &str, value: &str, cfg: &mut GasciiConfig) {
    match key {
        "sync_offset_ms" => {
            if let Some(v) = parse_sync_offset_ms(value) {
                cfg.sync_offset_ms = v;
            }
        }
        "sync_speed_mode" => {
            cfg.sync_speed_mode = parse_sync_speed_mode(value);
        }
        "sync_policy" => {
            cfg.sync_policy = parse_sync_policy(value);
        }
        "sync_hard_snap_ms" => {
            if let Some(v) = parse_sync_hard_snap_ms(value) {
                cfg.sync_hard_snap_ms = v;
            }
        }
        "sync_kp" => {
            if let Some(v) = parse_sync_kp(value) {
                cfg.sync_kp = v;
            }
        }
        "sync_profile_dir" => {
            if let Some(v) = parse_sync_profile_dir(value) {
                cfg.sync_profile_dir = v;
            }
        }
        "sync_profile_mode" => {
            cfg.sync_profile_mode = parse_sync_profile_mode(value);
        }
        "upscale_factor" => {
            if let Some(v) = parse_upscale_factor(value) {
                cfg.upscale_factor = v;
            }
        }
        "upscale_sharpen" => {
            if let Some(v) = parse_upscale_sharpen(value) {
                cfg.upscale_sharpen = v;
            }
        }
        "triangle_stride" | "tri_stride" => {
            if let Some(v) = parse_triangle_stride(value) {
                cfg.triangle_stride = v;
            }
        }
        "min_triangle_area_px2" | "tiny_triangle_area_px2" => {
            if let Some(v) = parse_min_triangle_area_px2(value) {
                cfg.min_triangle_area_px2 = v;
            }
        }
        _ => {}
    }
}
