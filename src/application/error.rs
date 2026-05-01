use crate::domain::{
    asset::errors::asset_error::AssetError, engine::errors::engine_error::EngineError,
    render::errors::render_error::RenderError, runtime::errors::runtime_error::RuntimeError,
};
use thiserror::Error;

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
