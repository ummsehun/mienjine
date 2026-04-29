use std::fmt::Debug;

pub trait EntityId: Clone + PartialEq + Eq + Debug + Send + Sync {}

impl<T> EntityId for T where T: Clone + PartialEq + Eq + Debug + Send + Sync {}

pub trait Entity: Debug + Send + Sync {
    type Id: EntityId;
    fn id(&self) -> &Self::Id;
}
