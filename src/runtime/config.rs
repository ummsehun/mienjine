//! Runtime configuration facade — re-exports types and `load_gascii_config`.

use std::fs;
use std::path::Path;

pub mod camera;
pub mod general;
pub mod sync;
pub mod types;
pub mod visual;

pub mod tests;

pub use types::{GasciiConfig, UiLanguage};

pub fn load_gascii_config(path: &Path) -> GasciiConfig {
    let Ok(content) = fs::read_to_string(path) else {
        return GasciiConfig::default();
    };
    let mut cfg = GasciiConfig::default();
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .replace(' ', "_");
        let value = raw_value.trim();

        general::apply_general(&key, value, &mut cfg);
        visual::apply_visual(&key, value, &mut cfg);
        camera::apply_camera(&key, value, &mut cfg);
        sync::apply_sync(&key, value, &mut cfg);
    }
    cfg
}
