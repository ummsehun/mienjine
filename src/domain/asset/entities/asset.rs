use crate::domain::asset::value_objects::{
    asset_format::AssetFormat, asset_metadata::AssetMetadata, asset_path::AssetPath,
};
use crate::domain::shared::{entity::Entity, ids::AssetId};

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
        Self {
            id,
            path,
            format,
            loaded: false,
            metadata: AssetMetadata::default(),
        }
    }

    pub fn path(&self) -> &AssetPath {
        &self.path
    }
    pub fn format(&self) -> &AssetFormat {
        &self.format
    }
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
    pub fn mark_loaded(&mut self) {
        self.loaded = true;
    }
    pub fn metadata(&self) -> &AssetMetadata {
        &self.metadata
    }
    pub fn set_metadata(&mut self, metadata: AssetMetadata) {
        self.metadata = metadata;
    }
}

impl Entity for Asset {
    type Id = AssetId;
    fn id(&self) -> &AssetId {
        &self.id
    }
}
