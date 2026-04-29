use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("asset not found: {path}")]
    NotFound { path: PathBuf },

    #[error("unsupported format: {format}")]
    UnsupportedFormat { format: String },

    #[error("corrupted asset: {reason}")]
    Corrupted { reason: String },

    #[error("loading failed: {reason}")]
    LoadingFailed { reason: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
