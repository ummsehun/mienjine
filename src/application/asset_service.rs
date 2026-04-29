use crate::domain::asset::{
    model::Asset,
    repository::AssetPort,
};
use crate::application::error::ApplicationError;
use std::path::Path;

pub struct AssetService<P: AssetPort> {
    port: P,
}

impl<P: AssetPort> AssetService<P> {
    pub fn new(port: P) -> Self {
        Self { port }
    }

    pub fn load_gltf(&self, path: &Path) -> Result<Asset, ApplicationError> {
        self.port.load_gltf(path).map_err(ApplicationError::from)
    }

    pub fn load_pmx(&self, path: &Path) -> Result<Asset, ApplicationError> {
        self.port.load_pmx(path).map_err(ApplicationError::from)
    }

    pub fn load_obj(&self, path: &Path) -> Result<Asset, ApplicationError> {
        self.port.load_obj(path).map_err(ApplicationError::from)
    }
}
