use crate::application::error::ApplicationError;
use crate::domain::engine::{
    entities::scene::Scene, repositories::scene_repository::SceneRepository,
};
use crate::domain::shared::ids::SceneId;

pub struct SceneService<R: SceneRepository> {
    repository: R,
}

impl<R: SceneRepository> SceneService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn load_scene(&self, id: SceneId) -> Result<Scene, ApplicationError> {
        self.repository.load(id).map_err(ApplicationError::from)
    }
}
