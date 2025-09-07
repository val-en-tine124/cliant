use thiserror::Error;


#[derive(Debug, Error)]
pub enum DomainError{
    #[error("Validate error: {0}")]
    ValidateError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Network connect error: {0}")]
    NetworkConnectError(String),
    #[error("Network timeout error: {0}")]
    NetworkTimeoutError(String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("An unexpected error occurred: {message}")]
    Other { message: String },
}

unsafe impl Send for DomainError{}