#[derive(Debug)]
pub enum DomainError {
    Validation(String),
    Unauthorized(String),
    NotFound,
    Conflict(String),
    BadGateway(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for DomainError {
    fn from(e: anyhow::Error) -> Self {
        DomainError::Internal(e)
    }
}
