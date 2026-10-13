//! Image types and metadata. Distinguish raw full-chip, capsule, partial, etc.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageKind {
    RawFullChip,
    Capsule,
    PartialRegion,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub kind: ImageKind,
    pub notes: Vec<String>,
}

impl ImageMeta {
    pub fn from_path(path: &Path) -> crate::domain::Result<Self> {
        let data = std::fs::read(path)?;
        let size = data.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256 = hex::encode(hasher.finalize());

        // Minimal heuristic; full classification is in firmware/validation modules.
        let kind = if size == 32 * 1024 * 1024 {
            ImageKind::RawFullChip
        } else if data.windows(4).any(|w| w == b"_FVH" || w == b"FV\x00\x00") {
            ImageKind::Capsule // rough
        } else {
            ImageKind::Unknown
        };

        Ok(Self {
            path: path.display().to_string(),
            size,
            sha256,
            kind,
            notes: vec![],
        })
    }

    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}
