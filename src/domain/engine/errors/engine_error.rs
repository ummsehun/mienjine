use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("scene not found: {id}")]
    SceneNotFound { id: u64 },

    #[error("animation clip not found: {clip_id}")]
    AnimationNotFound { clip_id: String },

    #[error("invalid bone hierarchy: {reason}")]
    InvalidBoneHierarchy { reason: String },

    #[error("physics init failed: {reason}")]
    PhysicsInitFailed { reason: String },

    #[error("pipeline execution failed: {reason}")]
    PipelineFailed { reason: String },

    #[error("legacy engine failure: {message}")]
    LegacyFailure { message: String },

    #[error("scene conversion failed: {reason}")]
    SceneConversionFailed { reason: String },
}

impl From<anyhow::Error> for EngineError {
    fn from(err: anyhow::Error) -> Self {
        EngineError::LegacyFailure {
            message: err.to_string(),
        }
    }
}
