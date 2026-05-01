use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("pipeline not initialized")]
    PipelineNotInitialized,

    #[error("gpu device error: {reason}")]
    GpuDeviceError { reason: String },

    #[error("frame buffer overflow")]
    FrameBufferOverflow,

    #[error("unsupported resolution: {width}x{height}")]
    UnsupportedResolution { width: u32, height: u32 },

    #[error("backend error: {backend} - {detail}")]
    BackendError { backend: String, detail: String },

    #[error("renderer not available")]
    RendererNotAvailable,

    #[error("legacy render failure: {message}")]
    LegacyFailure { message: String },
}

impl From<anyhow::Error> for RenderError {
    fn from(err: anyhow::Error) -> Self {
        RenderError::LegacyFailure {
            message: err.to_string(),
        }
    }
}
