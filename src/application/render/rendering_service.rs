use crate::application::error::ApplicationError;
use crate::domain::render::{
    entities::render_pipeline::RenderPipeline, repositories::render_repository::RenderRepository,
};
use crate::domain::shared::ids::RenderId;

pub struct RenderService<R: RenderRepository> {
    repository: R,
}

impl<R: RenderRepository> RenderService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn create_pipeline(&self, id: RenderId) -> Result<RenderPipeline, ApplicationError> {
        self.repository
            .create_pipeline(id)
            .map_err(ApplicationError::from)
    }
}
