use thiserror::Error;
use anyhow::Error as anyhowError;

#[derive(Error, Debug)]
pub enum CliantError {
    #[error("Network connection error: {0}")]
    ReqwestClient(#[from] reqwest::Error,),

    #[error("Network connection error: {0}")]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),
    
    
    #[error("Filesystem error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Critical system failure: {0}")]
    Fatal(String),

    #[error("String Parsing Error: {0}")]
    ParseError(String),
    ///This will convert anyhow::Error to my error variants.
    #[error("An error occurred: {0}")]
    Error(#[from] anyhowError)
}