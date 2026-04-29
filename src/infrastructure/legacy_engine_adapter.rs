use crate::domain::engine::{error::EngineError, model::Scene, repository::SceneRepository};
use crate::domain::shared::ids::SceneId;

/// Legacy engine adapter
/// Wraps existing src/engine/ functions to implement the new SceneRepository trait
pub struct LegacyEngineAdapter;

impl LegacyEngineAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl SceneRepository for LegacyEngineAdapter {
    fn load(&self, _id: SceneId) -> Result<Scene, EngineError> {
        todo!("LegacyEngineAdapter::load - bridge to existing engine/scene")
    }

    fn save(&self, _scene: &Scene) -> Result<(), EngineError> {
        todo!("LegacyEngineAdapter::save")
    }
}
