use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::asset::{
    entities::asset::Asset,
    errors::asset_error::AssetError,
    repositories::asset_repository::{AssetPort, AssetRepository},
    value_objects::{
        asset_format::AssetFormat, asset_metadata::AssetMetadata, asset_path::AssetPath,
    },
};
use crate::domain::shared::ids::AssetId;

static NEXT_ASSET_ID: AtomicU64 = AtomicU64::new(1);

pub struct LegacyAssetAdapter;

impl Default for LegacyAssetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacyAssetAdapter {
    pub fn new() -> Self {
        Self
    }

    fn extract_metadata(scene: &crate::scene::SceneCpu, path: &Path) -> AssetMetadata {
        let file_size_bytes = std::fs::metadata(path).ok().map(|m| m.len());
        AssetMetadata {
            vertex_count: scene.total_vertices(),
            triangle_count: scene.total_triangles(),
            animation_count: scene.animations.len(),
            file_size_bytes,
        }
    }

    fn build_asset(path: &Path, scene: &crate::scene::SceneCpu) -> Asset {
        let id = AssetId::new(NEXT_ASSET_ID.fetch_add(1, Ordering::Relaxed));
        let asset_path = AssetPath(path.to_path_buf());
        let format = AssetFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string(),
        );
        let mut asset = Asset::new(id, asset_path, format);
        asset.set_metadata(Self::extract_metadata(scene, path));
        asset.mark_loaded();
        asset
    }
}

impl AssetPort for LegacyAssetAdapter {
    fn load_gltf(&self, path: &Path) -> Result<Asset, AssetError> {
        let scene = crate::assets::loader::load_gltf(path).map_err(AssetError::from)?;
        Ok(Self::build_asset(path, &scene))
    }

    fn load_pmx(&self, path: &Path) -> Result<Asset, AssetError> {
        let scene = crate::assets::loader::load_pmx(path).map_err(AssetError::from)?;
        Ok(Self::build_asset(path, &scene))
    }

    fn load_obj(&self, path: &Path) -> Result<Asset, AssetError> {
        let scene = crate::assets::loader::load_obj(path).map_err(AssetError::from)?;
        Ok(Self::build_asset(path, &scene))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::asset::asset_loader_service::AssetService;
    use tempfile::TempDir;

    fn write_minimal_gltf(dir: &std::path::Path) -> std::path::PathBuf {
        let gltf_path = dir.join("test.gltf");
        let bin_path = dir.join("buf.bin");
        let mut buf = Vec::new();
        for v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in &[0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in &[0u16, 1, 2] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(&bin_path, &buf).expect("write bin");
        let gltf = format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "buf.bin", "byteLength": {}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 36, "target": 34962}},
    {{"buffer": 0, "byteOffset": 72, "byteLength": 6, "target": 34963}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}},
    {{"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1}}, "indices": 2}}]}}],
  "nodes": [{{"mesh": 0}}],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
            buf.len()
        );
        std::fs::write(&gltf_path, gltf).expect("write gltf");
        gltf_path
    }

    #[test]
    fn asset_adapter_loads_gltf_with_metadata() {
        let dir = TempDir::new().expect("create temp dir");
        let path = write_minimal_gltf(dir.path());

        let adapter = LegacyAssetAdapter::new();
        let asset = adapter.load_gltf(&path).expect("load gltf");

        assert!(asset.is_loaded());
        assert!(asset.metadata().vertex_count > 0);
        assert!(asset.metadata().triangle_count > 0);
        assert!(asset.metadata().file_size_bytes.is_some());
    }

    #[test]
    fn asset_service_with_legacy_adapter() {
        let dir = TempDir::new().expect("create temp dir");
        let path = write_minimal_gltf(dir.path());

        let adapter = LegacyAssetAdapter::new();
        let service = AssetService::new(adapter);
        let asset = service.load_gltf(&path).expect("service load");

        assert!(asset.is_loaded());
        assert_eq!(asset.metadata().triangle_count, 1);
    }
}
