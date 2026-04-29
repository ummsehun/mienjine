use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid identifier: {0}")]
    InvalidId(String),

    #[error("invariant violated: {reason}")]
    InvariantViolation { reason: String },

    #[error("validation failed: {field} = {value}")]
    ValidationFailed { field: String, value: String },
}
