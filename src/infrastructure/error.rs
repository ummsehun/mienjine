use thiserror::Error;

#[derive(Debug, Error)]
pub enum InfrastructureError {
    #[error("file system error: {0}")]
    FileSystem(#[from] std::io::Error),

    #[error("process execution failed: {message}")]
    ProcessExecution { message: String },

    #[error("legacy engine error: {message}")]
    LegacyEngine { message: String },

    #[error("legacy render error: {message}")]
    LegacyRender { message: String },

    #[error("legacy asset error: {message}")]
    LegacyAsset { message: String },

    #[error("adapter conversion failed: {reason}")]
    AdapterConversion { reason: String },
}
