use std::sync::Mutex;

use crate::domain::runtime::errors::runtime_error::RuntimeError;
use crate::domain::runtime::value_objects::sync_offset_ms::SyncOffsetMs;

const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;

#[derive(Debug)]
pub struct SyncState {
    offset: SyncOffsetMs,
    drift_ema: f32,
    hard_snap_count: u32,
    initialized: bool,
}

impl Default for SyncState {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            offset: SyncOffsetMs(0),
            drift_ema: 0.0,
            hard_snap_count: 0,
            initialized: false,
        }
    }

    pub fn with_offset(offset_ms: i32) -> Self {
        let mut state = Self::new();
        state.offset = SyncOffsetMs(offset_ms.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS));
        state.initialized = true;
        state
    }

    pub fn offset_ms(&self) -> i32 {
        self.offset.0
    }

    pub fn drift_ema(&self) -> f32 {
        self.drift_ema
    }

    pub fn hard_snap_count(&self) -> u32 {
        self.hard_snap_count
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn adjust_offset_delta(&mut self, delta_ms: i32) {
        self.offset.0 =
            (self.offset.0 + delta_ms).clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
        self.initialized = true;
    }

    pub fn reset_offset(&mut self) {
        self.offset = SyncOffsetMs(0);
        self.initialized = true;
    }

    pub fn set_offset(&mut self, new: SyncOffsetMs) {
        self.offset.0 = new.0.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
        self.initialized = true;
    }

    pub fn record_drift(&mut self, drift_ms: f32, alpha: f32) {
        self.drift_ema = self.drift_ema * (1.0 - alpha) + drift_ms * alpha;
    }

    pub fn record_hard_snap(&mut self) {
        self.hard_snap_count += 1;
    }
}

/// Thread-safe sync controller — the domain-level entry point for all sync operations.
///
/// Uses `Mutex` internally to protect shared state, making it safe for concurrent access
/// from multiple threads (e.g. render loop + input handler + HTTP sync endpoint).
#[derive(Debug)]
pub struct SyncController {
    state: Mutex<SyncState>,
}

impl Default for SyncController {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SyncState::new()),
        }
    }

    pub fn with_initial_offset(offset_ms: i32) -> Self {
        Self {
            state: Mutex::new(SyncState::with_offset(offset_ms)),
        }
    }

    pub fn offset_ms(&self) -> Result<i32, RuntimeError> {
        let state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        Ok(state.offset_ms())
    }

    pub fn drift_ema(&self) -> Result<f32, RuntimeError> {
        let state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        Ok(state.drift_ema())
    }

    pub fn hard_snap_count(&self) -> Result<u32, RuntimeError> {
        let state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        Ok(state.hard_snap_count())
    }

    pub fn adjust_offset_delta(&self, delta_ms: i32) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.adjust_offset_delta(delta_ms);
        Ok(())
    }

    pub fn reset_offset(&self) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.reset_offset();
        Ok(())
    }

    pub fn set_offset(&self, new: SyncOffsetMs) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.set_offset(new);
        Ok(())
    }

    pub fn record_drift(&self, drift_ms: f32, alpha: f32) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.record_drift(drift_ms, alpha);
        Ok(())
    }

    pub fn record_hard_snap(&self) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        state.record_hard_snap();
        Ok(())
    }

    pub fn snapshot(&self) -> Result<(i32, f32, u32), RuntimeError> {
        let state = self.state.lock().map_err(|_| RuntimeError::SyncFailed {
            reason: "sync state poisoned".to_string(),
        })?;
        Ok((
            state.offset_ms(),
            state.drift_ema(),
            state.hard_snap_count(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_starts_at_zero() {
        let controller = SyncController::new();
        assert_eq!(controller.offset_ms().unwrap(), 0);
        assert_eq!(controller.drift_ema().unwrap(), 0.0);
        assert_eq!(controller.hard_snap_count().unwrap(), 0);
    }

    #[test]
    fn adjust_offset_delta_clamps() {
        let controller = SyncController::new();
        controller.adjust_offset_delta(6000).unwrap();
        assert_eq!(controller.offset_ms().unwrap(), 5000);

        controller.adjust_offset_delta(-12000).unwrap();
        assert_eq!(controller.offset_ms().unwrap(), -5000);
    }

    #[test]
    fn reset_offset_clears_to_zero() {
        let controller = SyncController::with_initial_offset(500);
        controller.reset_offset().unwrap();
        assert_eq!(controller.offset_ms().unwrap(), 0);
    }

    #[test]
    fn record_drift_updates_ema() {
        let controller = SyncController::new();
        controller.record_drift(100.0, 0.5).unwrap();
        assert_eq!(controller.drift_ema().unwrap(), 50.0);
    }

    #[test]
    fn record_hard_snap_increments() {
        let controller = SyncController::new();
        controller.record_hard_snap().unwrap();
        controller.record_hard_snap().unwrap();
        assert_eq!(controller.hard_snap_count().unwrap(), 2);
    }

    #[test]
    fn snapshot_returns_consistent_state() {
        let controller = SyncController::with_initial_offset(120);
        controller.record_drift(30.0, 1.0).unwrap();
        controller.record_hard_snap().unwrap();

        let (offset, drift, snaps) = controller.snapshot().unwrap();
        assert_eq!(offset, 120);
        assert_eq!(drift, 30.0);
        assert_eq!(snaps, 1);
    }
}
