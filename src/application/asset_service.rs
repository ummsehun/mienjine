use crate::domain::asset::{model::Asset, repository::AssetRepository};
use crate::domain::shared::ids::AssetId;
use crate::application::error::ApplicationError;

pub struct AssetService<R: AssetRepository> {
    repository: R,
}

impl<R: AssetRepository> AssetService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn load_asset(&self, id: AssetId) -> Result<Asset, ApplicationError> {
        self.repository.load(id).map_err(ApplicationError::from)
    }
}
