use thiserror::Error;

#[derive(Error, Debug)]
pub enum SmbiosError {
    #[error("Windows API error: {0}")]
    WinApi(String),

    #[error("buffer too small or allocation failed: {0}")]
    Buffer(String),

    #[error("malformed SMBIOS data: {0}")]
    Malformed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported platform: {0}")]
    Unsupported(String),

    #[error("parse bounds error at offset {offset}: {msg}")]
    Bounds { offset: usize, msg: String },

    #[error("write blocked by safety gate: {0}")]
    WriteBlocked(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SmbiosError>;
