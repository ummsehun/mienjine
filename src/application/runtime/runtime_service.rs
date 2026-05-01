use crate::application::error::ApplicationError;
use crate::domain::runtime::entities::app_session::AppSession;
use crate::domain::shared::ids::SessionId;

pub struct RuntimeService;

impl Default for RuntimeService {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeService {
    pub fn new() -> Self {
        Self
    }

    pub fn create_session(&self, id: SessionId) -> Result<AppSession, ApplicationError> {
        Ok(AppSession::new(id))
    }
}
