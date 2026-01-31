/// Common error type for cssh.
#[derive(Debug, thiserror::Error)]
pub enum CsshError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Invalid message length: {0}")]
    InvalidMessageLength(u32),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Command execution error: {0}")]
    Execution(String),

    #[error("Authentication failed")]
    AuthenticationFailed,
}

pub type Result<T> = std::result::Result<T, CsshError>;
