//! Image types and metadata. Distinguish raw full-chip, capsule, and unknown.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use crate::domain::{BiosToolError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageKind {
    RawFullChip,
    Capsule,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub kind: ImageKind,
    pub notes: Vec<String>,
}

impl ImageMeta {
    pub fn from_path(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        let size = data.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256 = hex::encode(hasher.finalize());

        let kind = if data.len() >= 16 && &data[0..4] == b"\x8B\xA6\x3C\x7F" {
            // crude capsule signature placeholder
            ImageKind::Capsule
        } else if size >= 1024 * 1024 {
            ImageKind::RawFullChip
        } else {
            ImageKind::Unknown
        };

        Ok(Self {
            path: path.to_path_buf(),
            size,
            sha256,
            kind,
            notes: vec![],
        })
    }
}
