use thiserror::Error; // A popular crate for defining errors

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
}