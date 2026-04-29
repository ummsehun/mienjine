use crate::domain::runtime::error::RuntimeError;
use crate::domain::runtime::model::SyncOffsetMs;
use std::sync::Mutex;

#[derive(Debug)]
pub struct SyncState {
    offset: SyncOffsetMs,
    drift_ema: f32,
    initialized: bool,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            offset: SyncOffsetMs(0),
            drift_ema: 0.0,
            initialized: false,
        }
    }

    pub fn offset(&self) -> &SyncOffsetMs { &self.offset }
    pub fn adjust_offset(&mut self, new: SyncOffsetMs) { self.offset = new; }
    pub fn record_drift(&mut self, drift_ms: f32, alpha: f32) {
        self.drift_ema = self.drift_ema * (1.0 - alpha) + drift_ms * alpha;
    }
}

#[derive(Debug)]
pub struct SyncController {
    state: Mutex<SyncState>,
}

impl SyncController {
    pub fn new() -> Self {
        Self { state: Mutex::new(SyncState::new()) }
    }

    pub fn adjust_offset(&self, new: SyncOffsetMs) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.adjust_offset(new);
        Ok(())
    }
}
