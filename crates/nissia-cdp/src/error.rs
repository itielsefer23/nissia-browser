use thiserror::Error;

#[derive(Debug, Error)]
pub enum CdpTransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("CDP command '{method}' failed (code {code}): {message}")]
    CommandFailed {
        method: String,
        code: i64,
        message: String,
    },

    #[error("Failed to deserialize response for '{method}': {message}")]
    DeserializationFailed { method: String, message: String },

    #[error("Timeout waiting for event '{method}' after {timeout_ms}ms")]
    Timeout { method: String, timeout_ms: u64 },

    #[error("Browser not found: {0}")]
    BrowserNotFound(String),
}

pub type CdpResult<T> = Result<T, CdpTransportError>;
