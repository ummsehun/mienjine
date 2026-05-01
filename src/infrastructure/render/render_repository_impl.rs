use crate::domain::render::{
    entities::render_pipeline::RenderPipeline, errors::render_error::RenderError,
    repositories::render_repository::RenderRepository,
    value_objects::render_target_spec::RenderTargetSpec,
};
use crate::domain::shared::ids::RenderId;

pub struct LegacyRenderAdapter {
    default_spec: RenderTargetSpec,
}

impl Default for LegacyRenderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacyRenderAdapter {
    pub fn new() -> Self {
        Self {
            default_spec: RenderTargetSpec {
                width: 640,
                height: 360,
            },
        }
    }

    pub fn with_spec(width: u32, height: u32) -> Self {
        Self {
            default_spec: RenderTargetSpec { width, height },
        }
    }
}

impl RenderRepository for LegacyRenderAdapter {
    fn create_pipeline(&self, id: RenderId) -> Result<RenderPipeline, RenderError> {
        Ok(RenderPipeline::new(id, self.default_spec.clone()))
    }

    fn destroy_pipeline(&self, _id: RenderId) -> Result<(), RenderError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::render::rendering_service::RenderService;
    use crate::domain::shared::entity::Entity;

    #[test]
    fn render_adapter_creates_pipeline_with_default_spec() {
        let adapter = LegacyRenderAdapter::new();
        let id = RenderId::new(1);
        let pipeline = adapter.create_pipeline(id).expect("create pipeline");

        assert_eq!(pipeline.spec().width, 640);
        assert_eq!(pipeline.spec().height, 360);
    }

    #[test]
    fn render_adapter_with_custom_spec() {
        let adapter = LegacyRenderAdapter::with_spec(1280, 720);
        let id = RenderId::new(1);
        let pipeline = adapter.create_pipeline(id).expect("create pipeline");

        assert_eq!(pipeline.spec().width, 1280);
        assert_eq!(pipeline.spec().height, 720);
    }

    #[test]
    fn render_service_with_legacy_adapter() {
        let adapter = LegacyRenderAdapter::new();
        let service = RenderService::new(adapter);
        let id = RenderId::new(42);
        let pipeline = service.create_pipeline(id).expect("service create");

        assert_eq!(pipeline.id().0, 42);
    }

    #[test]
    fn render_adapter_destroy_pipeline() {
        let adapter = LegacyRenderAdapter::new();
        let id = RenderId::new(1);
        assert!(adapter.destroy_pipeline(id).is_ok());
    }
}
