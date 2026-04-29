use crate::domain::shared::{entity::Entity, ids::AssetId, value_object::ValueObject};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct AssetPath(pub PathBuf);

impl ValueObject for AssetPath {}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetFormat(pub String);

impl ValueObject for AssetFormat {}

#[derive(Debug)]
pub struct Asset {
    id: AssetId,
    path: AssetPath,
    format: AssetFormat,
    loaded: bool,
}

impl Asset {
    pub fn new(id: AssetId, path: AssetPath, format: AssetFormat) -> Self {
        Self { id, path, format, loaded: false }
    }

    pub fn path(&self) -> &AssetPath { &self.path }
    pub fn format(&self) -> &AssetFormat { &self.format }
    pub fn is_loaded(&self) -> bool { self.loaded }
    pub fn mark_loaded(&mut self) { self.loaded = true; }
}

impl Entity for Asset {
    type Id = AssetId;
    fn id(&self) -> &AssetId { &self.id }
}
