use crate::domain::render::entities::render_pipeline::RenderPipeline;
use crate::domain::render::errors::render_error::RenderError;
use crate::domain::shared::ids::RenderId;

pub trait RenderRepository: Send + Sync {
    fn create_pipeline(&self, id: RenderId) -> Result<RenderPipeline, RenderError>;
    fn destroy_pipeline(&self, id: RenderId) -> Result<(), RenderError>;
}
