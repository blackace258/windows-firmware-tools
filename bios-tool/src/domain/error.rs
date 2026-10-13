//! Typed errors for safety boundaries.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BiosToolError {
    #[error("board identity uncertain: {0}")]
    BoardUncertain(String),

    #[error("flash device/capacity uncertain: {0}")]
    ChipUncertain(String),

    #[error("backup missing, wrong-sized, or unverified: {0}")]
    BackupInvalid(String),

    #[error("image format ambiguous or incompatible: {0}")]
    ImageInvalid(String),

    #[error("board/image compatibility unproven: {0}")]
    CompatibilityFailed(String),

    #[error("programmer/backend unsupported or uncertain: {0}")]
    BackendUnsupported(String),

    #[error("protection, mismatch, or unsupported device reported by backend: {0}")]
    BackendProtection(String),

    #[error("no recovery method confirmed")]
    NoRecovery,

    #[error("pre-write validation failed: {0}")]
    PreWriteValidation(String),

    #[error("post-write verification failed: {0}")]
    PostWriteVerification(String),

    #[error("write blocked by safety state machine: {0}")]
    WriteBlocked(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("process error: {0}")]
    Process(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("size/layout mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    #[error("hash mismatch")]
    HashMismatch,

    #[error("overwrite refused (use --force): {0}")]
    OverwriteRefused(String),

    #[error("dry-run: operation not executed")]
    DryRun,

    #[error("other: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BiosToolError>;
