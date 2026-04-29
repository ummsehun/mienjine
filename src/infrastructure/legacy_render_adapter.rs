use crate::domain::render::{error::RenderError, model::RenderPipeline, repository::RenderRepository};
use crate::domain::shared::ids::RenderId;

/// Legacy render adapter
/// Wraps existing src/render/ functions to implement the new RenderRepository trait
pub struct LegacyRenderAdapter;

impl LegacyRenderAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl RenderRepository for LegacyRenderAdapter {
    fn create_pipeline(&self, _id: RenderId) -> Result<RenderPipeline, RenderError> {
        todo!("LegacyRenderAdapter::create_pipeline - bridge to existing render/renderer")
    }

    fn destroy_pipeline(&self, _id: RenderId) -> Result<(), RenderError> {
        todo!("LegacyRenderAdapter::destroy_pipeline")
    }
}
