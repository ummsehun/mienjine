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

    #[error("GLB parse error: {message}")]
    GlbParseError { message: String },

    #[error("PMX parse error: {message}")]
    PmxParseError { message: String },

    #[error("OBJ parse error: {message}")]
    ObjParseError { message: String },

    #[error("VMD parse error: {message}")]
    VmdParseError { message: String },

    #[error("legacy adapter failure: {message}")]
    LegacyFailure { message: String },
}

impl From<anyhow::Error> for AssetError {
    fn from(err: anyhow::Error) -> Self {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return AssetError::Io(std::io::Error::new(io_err.kind(), io_err.to_string()));
        }

        let msg = err.to_string();
        let msg_lower = msg.to_lowercase();
        if msg_lower.contains("glb") || msg_lower.contains("gltf") {
            AssetError::GlbParseError { message: msg }
        } else if msg_lower.contains("pmx") {
            AssetError::PmxParseError { message: msg }
        } else if msg_lower.contains("obj") {
            AssetError::ObjParseError { message: msg }
        } else if msg_lower.contains("vmd") {
            AssetError::VmdParseError { message: msg }
        } else {
            AssetError::LegacyFailure { message: msg }
        }
    }
}
