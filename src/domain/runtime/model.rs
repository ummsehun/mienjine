use crate::domain::shared::{entity::Entity, ids::SessionId, value_object::ValueObject};

#[derive(Debug, Clone, PartialEq)]
pub struct SyncOffsetMs(pub i32);

impl ValueObject for SyncOffsetMs {}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncProfile {
    pub key: String,
    pub offset_ms: SyncOffsetMs,
}

impl ValueObject for SyncProfile {}

#[derive(Debug)]
pub struct AppSession {
    id: SessionId,
    running: bool,
}

impl AppSession {
    pub fn new(id: SessionId) -> Self {
        Self { id, running: false }
    }

    pub fn start(&mut self) { self.running = true; }
    pub fn stop(&mut self) { self.running = false; }
    pub fn is_running(&self) -> bool { self.running }
}

impl Entity for AppSession {
    type Id = SessionId;
    fn id(&self) -> &SessionId { &self.id }
}
