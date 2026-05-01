use crate::domain::shared::{entity::Entity, ids::SceneId, value_object::ValueObject};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneName(pub String);

impl ValueObject for SceneName {}

/// Metadata derived from SceneCpu
#[derive(Debug, Clone, Default)]
pub struct SceneMetadata {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub animation_count: usize,
    pub mesh_count: usize,
    pub material_count: usize,
}

#[derive(Debug)]
pub struct Scene {
    id: SceneId,
    name: SceneName,
    metadata: SceneMetadata,
}

impl Scene {
    pub fn new(id: SceneId, name: SceneName) -> Self {
        Self { id, name, metadata: SceneMetadata::default() }
    }

    pub fn with_metadata(mut self, metadata: SceneMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn name(&self) -> &SceneName { &self.name }
    pub fn metadata(&self) -> &SceneMetadata { &self.metadata }
}

impl Entity for Scene {
    type Id = SceneId;
    fn id(&self) -> &SceneId { &self.id }
}
