use crate::domain::runtime::entities::app_session::AppSession;
use crate::domain::runtime::errors::runtime_error::RuntimeError;
use crate::domain::shared::ids::SessionId;

pub trait SessionRepository: Send + Sync {
    fn create_session(&self, id: SessionId) -> Result<AppSession, RuntimeError>;
    fn get_session(&self, id: SessionId) -> Result<AppSession, RuntimeError>;
    fn stop_session(&self, id: SessionId) -> Result<(), RuntimeError>;
}
