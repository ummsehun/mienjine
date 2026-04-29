use thiserror::Error;
use crate::domain::{
    asset::error::AssetError,
    engine::error::EngineError,
    render::error::RenderError,
    runtime::error::RuntimeError,
};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Asset(#[from] AssetError),

    #[error(transparent)]
    Engine(#[from] EngineError),

    #[error(transparent)]
    Render(#[from] RenderError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error("validation failed: {code}")]
    ValidationFailed { code: String },

    #[error("use case not supported: {name}")]
    NotSupported { name: String },
}
