use std::fmt;

/// Errors that can occur while supervising or verifying the local
/// `llama-server` process.
#[derive(Debug)]
pub enum RunnerError {
    /// A filesystem or I/O operation failed.
    Io(String),
    /// The configuration file was missing, malformed, or invalid.
    Config(String),
    /// Spawning, signaling, or waiting on the `llama-server` process failed.
    Process(String),
    /// An HTTP request to `llama-server` failed or returned an unexpected status.
    Http(String),
    /// A JSON payload could not be encoded or decoded.
    Json(String),
    /// An operation did not complete within its allotted time.
    Timeout(String),
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerError::Io(msg) => write!(f, "I/O error: {msg}"),
            RunnerError::Config(msg) => write!(f, "config error: {msg}"),
            RunnerError::Process(msg) => write!(f, "process error: {msg}"),
            RunnerError::Http(msg) => write!(f, "HTTP error: {msg}"),
            RunnerError::Json(msg) => write!(f, "JSON error: {msg}"),
            RunnerError::Timeout(msg) => write!(f, "timeout: {msg}"),
        }
    }
}

impl std::error::Error for RunnerError {}
