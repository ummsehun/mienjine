use crate::domain::asset::{error::AssetError, model::Asset};
use crate::domain::shared::ids::AssetId;

pub trait AssetRepository: Send + Sync {
    fn load(&self, id: AssetId) -> Result<Asset, AssetError>;
    fn preload(&self, ids: &[AssetId]) -> Result<Vec<Asset>, AssetError>;
    fn evict(&self, id: AssetId) -> Result<(), AssetError>;
}
