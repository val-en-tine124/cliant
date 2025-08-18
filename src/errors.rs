use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliantError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    #[error("User cancelled the operation.")]
    UserCancelled,

    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("An unexpected error occurred: {message}")]
    Other { message: String },
}

impl From<anyhow::Error> for CliantError {
    fn from(error: anyhow::Error) -> Self {
        CliantError::Other { message: error.to_string() }
    }
}

impl CliantError {
    pub fn exit_code(&self) -> exitcode::ExitCode {
        match self {
            CliantError::Network(_) => exitcode::UNAVAILABLE,
            CliantError::FileSystem(_) => exitcode::IOERR,
            CliantError::UserCancelled => exitcode::TEMPFAIL,
            CliantError::UrlParse(_) => exitcode::DATAERR,
            CliantError::Other { .. } => exitcode::SOFTWARE,
        }
    }
}
