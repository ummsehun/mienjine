use crate::domain::engine::{error::EngineError, model::Scene, repository::SceneRepository};
use crate::domain::shared::ids::SceneId;

pub struct LegacyEngineAdapter;

impl LegacyEngineAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl SceneRepository for LegacyEngineAdapter {
    fn load(&self, _id: SceneId) -> Result<Scene, EngineError> {
        Err(EngineError::LegacyFailure {
            message: "SceneRepository::load via LegacyEngineAdapter is not yet implemented. SceneCpu -> Scene mapping pending (Phase 1.5-D).".to_string(),
        })
    }

    fn save(&self, _scene: &Scene) -> Result<(), EngineError> {
        Err(EngineError::LegacyFailure {
            message: "save not implemented".to_string(),
        })
    }
}
