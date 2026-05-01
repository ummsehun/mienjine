use std::collections::HashMap;
use std::sync::Mutex;

use crate::domain::runtime::entities::app_session::AppSession;
use crate::domain::runtime::errors::runtime_error::RuntimeError;
use crate::domain::runtime::repositories::session_repository::SessionRepository;
use crate::domain::shared::ids::SessionId;

pub struct InMemorySessionRepository {
    sessions: Mutex<HashMap<SessionId, AppSession>>,
}

impl Default for InMemorySessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl SessionRepository for InMemorySessionRepository {
    fn create_session(&self, id: SessionId) -> Result<AppSession, RuntimeError> {
        let mut sessions = self.sessions.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "failed to lock session store".to_string(),
        })?;

        if sessions.contains_key(&id) {
            return Err(RuntimeError::InvalidStateTransition {
                from: "none".to_string(),
                to: "duplicate session".to_string(),
            });
        }

        let session = AppSession::new(id);
        sessions.insert(id, session.clone());
        Ok(session)
    }

    fn get_session(&self, id: SessionId) -> Result<AppSession, RuntimeError> {
        let sessions = self.sessions.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "failed to lock session store".to_string(),
        })?;

        sessions
            .get(&id)
            .cloned()
            .ok_or_else(|| RuntimeError::SyncFailed {
                reason: format!("session {id} not found"),
            })
    }

    fn stop_session(&self, id: SessionId) -> Result<(), RuntimeError> {
        let mut sessions = self.sessions.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "failed to lock session store".to_string(),
        })?;

        sessions
            .get_mut(&id)
            .map(|s| s.stop())
            .ok_or_else(|| RuntimeError::SyncFailed {
                reason: format!("session {id} not found"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shared::entity::Entity;

    #[test]
    fn create_and_get_session() {
        let repo = InMemorySessionRepository::new();
        let id = SessionId::new(1);
        let session = repo.create_session(id).expect("create session");
        assert!(session.is_running() == false);

        let retrieved = repo.get_session(id).expect("get session");
        assert_eq!(retrieved.id(), session.id());
    }

    #[test]
    fn stop_session() {
        let repo = InMemorySessionRepository::new();
        let id = SessionId::new(2);
        repo.create_session(id).expect("create session");
        repo.stop_session(id).expect("stop session");

        let session = repo.get_session(id).expect("get session");
        assert!(!session.is_running());
    }

    #[test]
    fn duplicate_session_fails() {
        let repo = InMemorySessionRepository::new();
        let id = SessionId::new(3);
        repo.create_session(id).expect("create session");
        let result = repo.create_session(id);
        assert!(result.is_err());
    }
}
