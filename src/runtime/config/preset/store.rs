use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::helpers::{temp_path_for, unix_now_secs, validate_preset_name};
use super::types::{PresetFile, SavePresetResult, WizardPreset};

#[derive(Debug, Clone)]
pub struct PresetStore {
    path: PathBuf,
    file: PresetFile,
}

impl PresetStore {
    pub fn load_default() -> Result<Self> {
        let Some(path) = super::helpers::default_preset_store_path() else {
            bail!("failed to resolve user config directory")
        };
        Self::load(&path)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: path.to_path_buf(),
                    file: PresetFile::default(),
                });
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
        };

        let mut file: PresetFile =
            toml::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
        if file.schema_version == 0 {
            file.schema_version = super::types::PRESET_SCHEMA_VERSION;
        }

        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    pub fn list_names(&self) -> Vec<String> {
        self.file.presets.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<&WizardPreset> {
        self.file.presets.get(name)
    }

    pub fn default_preset(&self) -> Option<&str> {
        self.file.default_preset.as_deref()
    }

    pub fn last_used(&self) -> Option<&str> {
        self.file.last_used.as_deref()
    }

    pub fn has_preset(&self, name: &str) -> bool {
        self.file.presets.contains_key(name)
    }

    pub fn save_named(
        &mut self,
        name: &str,
        mut preset: WizardPreset,
        allow_overwrite: bool,
    ) -> Result<SavePresetResult> {
        let normalized = validate_preset_name(name)?;

        let result = if let Some(existing) = self.file.presets.get(&normalized) {
            if !allow_overwrite {
                return Ok(SavePresetResult::NameConflict);
            }
            preset.created_at_unix = existing.created_at_unix;
            SavePresetResult::Overwritten
        } else {
            SavePresetResult::Created
        };

        let now = unix_now_secs();
        if preset.created_at_unix == 0 {
            preset.created_at_unix = now;
        }
        preset.updated_at_unix = now;

        self.file.presets.insert(normalized.clone(), preset);
        self.file.last_used = Some(normalized.clone());
        if self
            .file
            .default_preset
            .as_deref()
            .is_some_and(|name| !self.file.presets.contains_key(name))
        {
            self.file.default_preset = None;
        }
        self.save_atomic()?;

        Ok(result)
    }

    pub fn set_last_used(&mut self, name: Option<String>) -> Result<()> {
        if let Some(ref key) = name
            && !self.file.presets.contains_key(key)
        {
            bail!("preset '{key}' not found")
        }
        self.file.last_used = name;
        self.save_atomic()
    }

    fn save_atomic(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let encoded = toml::to_string_pretty(&self.file).context("serialize preset file")?;
        let tmp = temp_path_for(&self.path);
        fs::write(&tmp, encoded).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_name_validation_rejects_invalid_chars() {
        assert!(validate_preset_name("safe_name-01").is_ok());
        assert!(validate_preset_name("bad/name").is_err());
        assert!(validate_preset_name("    ").is_err());
    }

    #[test]
    fn preset_store_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("presets.toml");

        let mut store = PresetStore::load(&path).expect("load default");
        let mut preset = WizardPreset::default();
        preset.render.mode = "ascii".to_owned();
        let result = store
            .save_named("balanced", preset.clone(), false)
            .expect("save preset");
        assert_eq!(result, SavePresetResult::Created);

        let loaded = PresetStore::load(&path).expect("reload");
        assert_eq!(loaded.last_used(), Some("balanced"));
        assert_eq!(
            loaded.get("balanced").map(|p| p.render.mode.as_str()),
            Some("ascii")
        );
    }

    #[test]
    fn preset_store_requires_overwrite_confirmation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("presets.toml");

        let mut store = PresetStore::load(&path).expect("load default");
        let result = store
            .save_named("locked", WizardPreset::default(), false)
            .expect("save first");
        assert_eq!(result, SavePresetResult::Created);

        let conflict = store
            .save_named("locked", WizardPreset::default(), false)
            .expect("save second");
        assert_eq!(conflict, SavePresetResult::NameConflict);

        let overwrite = store
            .save_named("locked", WizardPreset::default(), true)
            .expect("overwrite");
        assert_eq!(overwrite, SavePresetResult::Overwritten);
    }
}
