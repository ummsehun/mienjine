use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::scene::SyncSpeedMode;
use crate::shared::constants::SYNC_OFFSET_LIMIT_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProfileMode {
    Auto,
    Off,
    Write,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncProfileEntry {
    pub sync_offset_ms: i32,
    pub sync_hard_snap_ms: Option<u32>,
    pub sync_kp: Option<f32>,
    pub sync_speed_mode: Option<SyncSpeedMode>,
    pub updated_at_unix: u64,
}

impl SyncProfileEntry {
    pub fn with_offset(sync_offset_ms: i32) -> Self {
        Self {
            sync_offset_ms,
            sync_hard_snap_ms: None,
            sync_kp: None,
            sync_speed_mode: None,
            updated_at_unix: unix_now_secs(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SyncProfileStore {
    profiles: BTreeMap<String, SyncProfileEntry>,
}

impl SyncProfileStore {
    pub fn load(path: &Path) -> Result<Self> {
        let Ok(bytes) = fs::read(path) else {
            return Ok(Self::default());
        };
        let root: Value =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;

        let mut profiles = BTreeMap::new();
        let candidates = root
            .get("profiles")
            .and_then(Value::as_object)
            .or_else(|| root.as_object());
        if let Some(map) = candidates {
            for (key, raw) in map {
                if let Some(entry) = parse_entry(raw) {
                    profiles.insert(key.clone(), entry);
                }
            }
        }
        Ok(Self { profiles })
    }

    pub fn get(&self, key: &str) -> Option<&SyncProfileEntry> {
        self.profiles.get(key)
    }

    pub fn upsert(&mut self, key: String, entry: SyncProfileEntry) {
        self.profiles.insert(key, entry);
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }

        let mut profiles = serde_json::Map::new();
        for (key, entry) in &self.profiles {
            profiles.insert(key.clone(), serialize_entry(entry));
        }
        let root = Value::Object(serde_json::Map::from_iter([
            ("version".to_owned(), Value::from(1_u64)),
            ("profiles".to_owned(), Value::Object(profiles)),
        ]));
        let encoded = serde_json::to_vec_pretty(&root).context("serialize sync profiles")?;

        let tmp = temp_path_for(path);
        fs::write(&tmp, encoded).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }
}

pub fn default_profile_store_path(dir: &Path) -> PathBuf {
    dir.join("profiles.json")
}

pub fn build_profile_key(
    scene_kind: &str,
    primary: Option<&Path>,
    music: Option<&Path>,
    camera: Option<&Path>,
) -> String {
    format!(
        "scene={};source={};music={};camera={}",
        scene_kind,
        normalize_path(primary),
        normalize_path(music),
        normalize_path(camera)
    )
}

fn parse_entry(raw: &Value) -> Option<SyncProfileEntry> {
    let obj = raw.as_object()?;
    let sync_offset_ms = obj
        .get("sync_offset_ms")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .clamp(
            -(i64::from(SYNC_OFFSET_LIMIT_MS)),
            i64::from(SYNC_OFFSET_LIMIT_MS),
        ) as i32;
    let sync_hard_snap_ms = obj
        .get("sync_hard_snap_ms")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(10, 2_000) as u32);
    let sync_kp = obj
        .get("sync_kp")
        .and_then(Value::as_f64)
        .map(|value| (value as f32).clamp(0.01, 1.0));
    let sync_speed_mode = obj
        .get("sync_speed_mode")
        .and_then(Value::as_str)
        .map(|value| {
            let lower = value.to_ascii_lowercase();
            if lower.starts_with("real") || lower == "1x" {
                SyncSpeedMode::Realtime1x
            } else {
                SyncSpeedMode::AutoDurationFit
            }
        });
    let updated_at_unix = obj
        .get("updated_at_unix")
        .and_then(Value::as_u64)
        .unwrap_or_else(unix_now_secs);

    Some(SyncProfileEntry {
        sync_offset_ms,
        sync_hard_snap_ms,
        sync_kp,
        sync_speed_mode,
        updated_at_unix,
    })
}

fn serialize_entry(entry: &SyncProfileEntry) -> Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "sync_offset_ms".to_owned(),
        Value::from(entry.sync_offset_ms),
    );
    map.insert(
        "updated_at_unix".to_owned(),
        Value::from(entry.updated_at_unix),
    );
    if let Some(value) = entry.sync_hard_snap_ms {
        map.insert("sync_hard_snap_ms".to_owned(), Value::from(value));
    }
    if let Some(value) = entry.sync_kp {
        map.insert("sync_kp".to_owned(), Value::from(value));
    }
    if let Some(value) = entry.sync_speed_mode {
        let text = match value {
            SyncSpeedMode::AutoDurationFit => "auto",
            SyncSpeedMode::Realtime1x => "realtime",
        };
        map.insert("sync_speed_mode".to_owned(), Value::from(text));
    }
    Value::Object(map)
}

fn normalize_path(path: Option<&Path>) -> String {
    path.map(normalize_existing_path)
        .unwrap_or_else(|| "none".to_owned())
}

fn normalize_existing_path(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("profiles.json");
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!("{file_name}.{pid}.{nonce}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_key_normalizes_optional_paths() {
        let key = build_profile_key(
            "glb",
            Some(Path::new("assets\\glb\\miku.glb")),
            None,
            Some(Path::new("assets/camera/world.vmd")),
        );
        assert_eq!(
            key,
            "scene=glb;source=assets/glb/miku.glb;music=none;camera=assets/camera/world.vmd"
        );
    }

    #[test]
    fn profile_key_canonicalizes_equivalent_existing_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("scene.glb");
        fs::write(&path, b"glb").expect("write scene");
        let alt = dir.path().join(".").join("scene.glb");

        let key_a = build_profile_key("glb", Some(&path), None, None);
        let key_b = build_profile_key("glb", Some(&alt), None, None);

        assert_eq!(key_a, key_b);
    }

    #[test]
    fn profile_key_includes_scene_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("scene.obj");
        fs::write(&path, b"obj").expect("write scene");

        let glb_key = build_profile_key("glb", Some(&path), None, None);
        let obj_key = build_profile_key("obj", Some(&path), None, None);
        let cube_key = build_profile_key("cube", None, None, None);

        assert_ne!(glb_key, obj_key);
        assert_ne!(glb_key, cube_key);
        assert_ne!(obj_key, cube_key);
    }

    #[test]
    fn profile_key_preserves_none_for_missing_paths() {
        let key = build_profile_key("cube", None, None, None);
        assert_eq!(key, "scene=cube;source=none;music=none;camera=none");
    }

    #[test]
    fn profile_store_roundtrip_and_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("profiles.json");

        let mut store = SyncProfileStore::load(&path).expect("load default");
        assert!(store.get("a").is_none());

        let first = SyncProfileEntry {
            sync_offset_ms: 120,
            sync_hard_snap_ms: Some(180),
            sync_kp: Some(0.20),
            sync_speed_mode: Some(SyncSpeedMode::AutoDurationFit),
            updated_at_unix: 1,
        };
        store.upsert("a".to_owned(), first.clone());
        store.save_atomic(&path).expect("save first");

        let mut loaded = SyncProfileStore::load(&path).expect("reload");
        assert_eq!(loaded.get("a"), Some(&first));

        let second = SyncProfileEntry::with_offset(-80);
        loaded.upsert("b".to_owned(), second.clone());
        loaded.save_atomic(&path).expect("save second");

        let merged = SyncProfileStore::load(&path).expect("load merged");
        assert_eq!(merged.get("a"), Some(&first));
        assert_eq!(merged.get("b").map(|v| v.sync_offset_ms), Some(-80));
    }
}
