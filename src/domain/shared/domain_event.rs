use std::fmt::Debug;

pub trait DomainEvent: Debug + Clone + Send + Sync {
    fn event_type(&self) -> &'static str;
}
