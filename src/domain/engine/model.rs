use crate::domain::shared::{entity::Entity, ids::SceneId, value_object::ValueObject};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneName(pub String);

impl ValueObject for SceneName {}

#[derive(Debug)]
pub struct Scene {
    id: SceneId,
    name: SceneName,
}

impl Scene {
    pub fn new(id: SceneId, name: SceneName) -> Self {
        Self { id, name }
    }

    pub fn name(&self) -> &SceneName { &self.name }
}

impl Entity for Scene {
    type Id = SceneId;
    fn id(&self) -> &SceneId { &self.id }
}
