use crate::domain::shared::{entity::Entity, ids::SessionId};

#[derive(Debug, Clone)]
pub struct AppSession {
    id: SessionId,
    running: bool,
}

impl AppSession {
    pub fn new(id: SessionId) -> Self {
        Self { id, running: false }
    }

    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Entity for AppSession {
    type Id = SessionId;

    fn id(&self) -> &SessionId {
        &self.id
    }
}
