use crate::domain::engine::value_objects::{scene_metadata::SceneMetadata, scene_name::SceneName};
use crate::domain::shared::{entity::Entity, ids::SceneId};

#[derive(Debug)]
pub struct Scene {
    id: SceneId,
    name: SceneName,
    metadata: SceneMetadata,
}

impl Scene {
    pub fn new(id: SceneId, name: SceneName) -> Self {
        Self {
            id,
            name,
            metadata: SceneMetadata::default(),
        }
    }

    pub fn with_metadata(mut self, metadata: SceneMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn name(&self) -> &SceneName {
        &self.name
    }
    pub fn metadata(&self) -> &SceneMetadata {
        &self.metadata
    }
}

impl Entity for Scene {
    type Id = SceneId;

    fn id(&self) -> &SceneId {
        &self.id
    }
}
