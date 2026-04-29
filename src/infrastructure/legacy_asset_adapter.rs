use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::asset::{
    error::AssetError,
    model::{Asset, AssetFormat, AssetPath},
    repository::{AssetPort, AssetRepository},
};
use crate::domain::shared::ids::AssetId;

static NEXT_ASSET_ID: AtomicU64 = AtomicU64::new(1);

pub struct LegacyAssetAdapter;

impl LegacyAssetAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl AssetPort for LegacyAssetAdapter {
    fn load_gltf(&self, path: &Path) -> Result<Asset, AssetError> {
        let _scene = crate::assets::loader::load_gltf(path).map_err(AssetError::from)?;

        let id = AssetId::new(NEXT_ASSET_ID.fetch_add(1, Ordering::Relaxed));
        let asset_path = AssetPath(path.to_path_buf());
        let format = AssetFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("glb")
                .to_string(),
        );
        let mut asset = Asset::new(id, asset_path, format);
        asset.mark_loaded();
        Ok(asset)
    }

    fn load_pmx(&self, path: &Path) -> Result<Asset, AssetError> {
        let _scene = crate::assets::loader::load_pmx(path).map_err(AssetError::from)?;

        let id = AssetId::new(NEXT_ASSET_ID.fetch_add(1, Ordering::Relaxed));
        let asset_path = AssetPath(path.to_path_buf());
        let format = AssetFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("pmx")
                .to_string(),
        );
        let mut asset = Asset::new(id, asset_path, format);
        asset.mark_loaded();
        Ok(asset)
    }

    fn load_obj(&self, path: &Path) -> Result<Asset, AssetError> {
        let _scene = crate::assets::loader::load_obj(path).map_err(AssetError::from)?;

        let id = AssetId::new(NEXT_ASSET_ID.fetch_add(1, Ordering::Relaxed));
        let asset_path = AssetPath(path.to_path_buf());
        let format = AssetFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("obj")
                .to_string(),
        );
        let mut asset = Asset::new(id, asset_path, format);
        asset.mark_loaded();
        Ok(asset)
    }
}

impl AssetRepository for LegacyAssetAdapter {
    fn load(&self, _id: AssetId) -> Result<Asset, AssetError> {
        Err(AssetError::LegacyFailure {
            message: "AssetRepository::load requires path-based loading via AssetPort. Use load_gltf/load_pmx/load_obj instead.".to_string(),
        })
    }

    fn preload(&self, _ids: &[AssetId]) -> Result<Vec<Asset>, AssetError> {
        Ok(Vec::new())
    }

    fn evict(&self, _id: AssetId) -> Result<(), AssetError> {
        Ok(())
    }
}
