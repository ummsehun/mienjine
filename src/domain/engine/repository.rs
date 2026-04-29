use crate::domain::engine::{error::EngineError, model::Scene};
use crate::domain::shared::ids::SceneId;

pub trait SceneRepository: Send + Sync {
    fn load(&self, id: SceneId) -> Result<Scene, EngineError>;
    fn save(&self, scene: &Scene) -> Result<(), EngineError>;
}
