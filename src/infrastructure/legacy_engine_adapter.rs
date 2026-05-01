use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::engine::{
    error::EngineError,
    model::{Scene, SceneMetadata, SceneName},
    repository::SceneRepository,
};
use crate::domain::shared::ids::SceneId;

static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(1);

pub struct LegacyEngineAdapter {
    scenes: HashMap<SceneId, crate::scene::SceneCpu>,
}

impl LegacyEngineAdapter {
    pub fn new() -> Self {
        Self { scenes: HashMap::new() }
    }

    pub fn load_from_path(&mut self, path: &Path) -> Result<SceneId, EngineError> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let scene = match ext.as_str() {
            "glb" | "gltf" => crate::assets::loader::load_gltf(path),
            "pmx" => crate::assets::loader::load_pmx(path),
            "obj" => crate::assets::loader::load_obj(path),
            _ => return Err(EngineError::LegacyFailure {
                message: format!("unsupported format: {}", ext),
            }),
        }.map_err(|e| EngineError::LegacyFailure { message: e.to_string() })?;

        let id = SceneId::new(NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed));
        self.scenes.insert(id, scene);
        Ok(id)
    }

    fn scene_cpu_to_metadata(scene: &crate::scene::SceneCpu) -> SceneMetadata {
        SceneMetadata {
            vertex_count: scene.total_vertices(),
            triangle_count: scene.total_triangles(),
            animation_count: scene.animations.len(),
            mesh_count: scene.meshes.len(),
            material_count: scene.materials.len(),
        }
    }
}

impl SceneRepository for LegacyEngineAdapter {
    fn load(&self, id: SceneId) -> Result<Scene, EngineError> {
        let scene_cpu = self.scenes.get(&id).ok_or_else(|| EngineError::LegacyFailure {
            message: format!("scene not found: {}", id.0),
        })?;

        let name = scene_cpu
            .nodes
            .first()
            .and_then(|n| n.name.clone())
            .unwrap_or_else(|| "unnamed_scene".to_string());

        let metadata = Self::scene_cpu_to_metadata(scene_cpu);

        Ok(Scene::new(id, SceneName(name)).with_metadata(metadata))
    }

    fn save(&self, _scene: &Scene) -> Result<(), EngineError> {
        Err(EngineError::LegacyFailure {
            message: "save not yet implemented".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::scene_service::SceneService;
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
  "nodes": [{{"mesh": 0, "name": "TestNode"}}],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
            buf.len()
        );
        std::fs::write(&gltf_path, gltf).expect("write gltf");
        gltf_path
    }

    #[test]
    fn engine_adapter_loads_scene_from_path() {
        let dir = TempDir::new().expect("create temp dir");
        let path = write_minimal_gltf(dir.path());

        let mut adapter = LegacyEngineAdapter::new();
        let scene_id = adapter.load_from_path(&path).expect("load from path");

        let scene = adapter.load(scene_id).expect("load scene");
        assert_eq!(scene.metadata().triangle_count, 1);
        assert_eq!(scene.metadata().vertex_count, 3);
        assert_eq!(scene.name().0, "TestNode");
    }

    #[test]
    fn scene_service_with_legacy_adapter() {
        let dir = TempDir::new().expect("create temp dir");
        let path = write_minimal_gltf(dir.path());

        let mut adapter = LegacyEngineAdapter::new();
        let scene_id = adapter.load_from_path(&path).expect("load from path");
        let service = SceneService::new(adapter);
        let scene = service.load_scene(scene_id).expect("service load");

        assert!(scene.metadata().mesh_count > 0);
    }
}
