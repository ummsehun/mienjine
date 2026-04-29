use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("invalid cli arguments: {reason}")]
    InvalidCliArgs { reason: String },

    #[error("config parse error: {path}")]
    ConfigParseError { path: PathBuf, reason: String },

    #[error("sync failed: {reason}")]
    SyncFailed { reason: String },

    #[error("terminal not supported")]
    TerminalNotSupported,

    #[error("panic occurred: {message}")]
    PanicOccurred { message: String },

    #[error("invalid state transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },
}
