use crate::domain::shared::{entity::Entity, ids::AssetId, value_object::ValueObject};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct AssetPath(pub PathBuf);

impl ValueObject for AssetPath {}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetFormat(pub String);

impl ValueObject for AssetFormat {}

/// Metadata extracted from loaded scene data
#[derive(Debug, Clone, Default)]
pub struct AssetMetadata {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub animation_count: usize,
    pub file_size_bytes: Option<u64>,
}

#[derive(Debug)]
pub struct Asset {
    id: AssetId,
    path: AssetPath,
    format: AssetFormat,
    loaded: bool,
    metadata: AssetMetadata,
}

impl Asset {
    pub fn new(id: AssetId, path: AssetPath, format: AssetFormat) -> Self {
        Self { id, path, format, loaded: false, metadata: AssetMetadata::default() }
    }

    pub fn path(&self) -> &AssetPath { &self.path }
    pub fn format(&self) -> &AssetFormat { &self.format }
    pub fn is_loaded(&self) -> bool { self.loaded }
    pub fn mark_loaded(&mut self) { self.loaded = true; }
    pub fn metadata(&self) -> &AssetMetadata { &self.metadata }
    pub fn set_metadata(&mut self, metadata: AssetMetadata) { self.metadata = metadata; }
}

impl Entity for Asset {
    type Id = AssetId;
    fn id(&self) -> &AssetId { &self.id }
}
