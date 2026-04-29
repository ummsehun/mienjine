use crate::domain::asset::{error::AssetError, model::Asset};
use crate::domain::shared::ids::AssetId;
use std::path::Path;

pub trait AssetRepository: Send + Sync {
    fn load(&self, id: AssetId) -> Result<Asset, AssetError>;
    fn preload(&self, ids: &[AssetId]) -> Result<Vec<Asset>, AssetError>;
    fn evict(&self, id: AssetId) -> Result<(), AssetError>;
}

/// Asset Port - 외부 GLB/PMX/OBJ 로더와의 경계
pub trait AssetPort: Send + Sync {
    fn load_gltf(&self, path: &Path) -> Result<Asset, AssetError>;
    fn load_pmx(&self, path: &Path) -> Result<Asset, AssetError>;
    fn load_obj(&self, path: &Path) -> Result<Asset, AssetError>;
}
