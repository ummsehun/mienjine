use crate::domain::render::{
    error::RenderError,
    model::{RenderPipeline, RenderTargetSpec},
    repository::RenderRepository,
};
use crate::domain::shared::ids::RenderId;

pub struct LegacyRenderAdapter;

impl LegacyRenderAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl RenderRepository for LegacyRenderAdapter {
    fn create_pipeline(&self, id: RenderId) -> Result<RenderPipeline, RenderError> {
        let spec = RenderTargetSpec {
            width: 640,
            height: 360,
        };
        Ok(RenderPipeline::new(id, spec))
    }

    fn destroy_pipeline(&self, _id: RenderId) -> Result<(), RenderError> {
        Ok(())
    }
}
