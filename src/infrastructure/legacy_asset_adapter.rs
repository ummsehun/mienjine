use crate::domain::asset::{error::AssetError, model::Asset, repository::AssetRepository};
use crate::domain::shared::ids::AssetId;

/// Legacy asset loader adapter
/// Wraps existing src/assets/loader functions to implement the new AssetRepository trait
pub struct LegacyAssetAdapter;

impl LegacyAssetAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl AssetRepository for LegacyAssetAdapter {
    fn load(&self, _id: AssetId) -> Result<Asset, AssetError> {
        // TODO: Bridge to crate::assets::loader::load_gltf or load_pmx based on id
        todo!("LegacyAssetAdapter::load - bridge to existing asset loader")
    }

    fn preload(&self, _ids: &[AssetId]) -> Result<Vec<Asset>, AssetError> {
        todo!("LegacyAssetAdapter::preload")
    }

    fn evict(&self, _id: AssetId) -> Result<(), AssetError> {
        todo!("LegacyAssetAdapter::evict")
    }
}
