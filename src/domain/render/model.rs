use crate::domain::shared::{entity::Entity, ids::RenderId, value_object::ValueObject};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderTargetSpec {
    pub width: u32,
    pub height: u32,
}

impl ValueObject for RenderTargetSpec {}

#[derive(Debug)]
pub struct RenderPipeline {
    id: RenderId,
    spec: RenderTargetSpec,
}

impl RenderPipeline {
    pub fn new(id: RenderId, spec: RenderTargetSpec) -> Self {
        Self { id, spec }
    }

    pub fn spec(&self) -> &RenderTargetSpec { &self.spec }
}

impl Entity for RenderPipeline {
    type Id = RenderId;
    fn id(&self) -> &RenderId { &self.id }
}
