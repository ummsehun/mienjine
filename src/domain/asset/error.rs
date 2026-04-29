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

    #[error("legacy adapter failure: {message}")]
    LegacyFailure { message: String },
}

impl From<anyhow::Error> for AssetError {
    fn from(err: anyhow::Error) -> Self {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return AssetError::Io(std::io::Error::new(io_err.kind(), io_err.to_string()));
        }
        AssetError::LegacyFailure { message: err.to_string() }
    }
}
