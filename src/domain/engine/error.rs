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
}
