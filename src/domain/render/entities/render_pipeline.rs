use crate::domain::shared::{entity::Entity, ids::RenderId};

use crate::domain::render::value_objects::render_target_spec::RenderTargetSpec;

#[derive(Debug)]
pub struct RenderPipeline {
    id: RenderId,
    spec: RenderTargetSpec,
}

impl RenderPipeline {
    pub fn new(id: RenderId, spec: RenderTargetSpec) -> Self {
        Self { id, spec }
    }

    pub fn spec(&self) -> &RenderTargetSpec {
        &self.spec
    }
}

impl Entity for RenderPipeline {
    type Id = RenderId;

    fn id(&self) -> &RenderId {
        &self.id
    }
}
