use crate::domain::render::{error::RenderError, model::RenderPipeline};
use crate::domain::shared::ids::RenderId;

pub trait RenderRepository: Send + Sync {
    fn create_pipeline(&self, id: RenderId) -> Result<RenderPipeline, RenderError>;
    fn destroy_pipeline(&self, id: RenderId) -> Result<(), RenderError>;
}
