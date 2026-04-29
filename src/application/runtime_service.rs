use crate::domain::runtime::model::AppSession;
use crate::domain::shared::ids::SessionId;
use crate::application::error::ApplicationError;

pub struct RuntimeService;

impl RuntimeService {
    pub fn new() -> Self {
        Self
    }

    pub fn create_session(&self, id: SessionId) -> Result<AppSession, ApplicationError> {
        Ok(AppSession::new(id))
    }
}
