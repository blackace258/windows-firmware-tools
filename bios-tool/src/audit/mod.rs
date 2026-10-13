//! Operation logs and reports. UUID, UTC, versions, hashes, status. No secrets.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub operation_id: String,
    pub utc_timestamp: String,
    pub app_version: String,
    pub board_model: Option<String>,
    pub board_revision: Option<String>,
    pub chip_identity: Option<String>,
    pub chip_confidence: Option<String>,
    pub programmer: Option<String>,
    pub image_name: Option<String>,
    pub image_size: Option<u64>,
    pub image_sha256: Option<String>,
    pub validation_results: Vec<String>,
    pub backend_args: Option<Vec<String>>,
    pub backend_exit_code: Option<i32>,
    pub pre_write_backup_hash: Option<String>,
    pub candidate_hash: Option<String>,
    pub post_write_hash: Option<String>,
    pub final_status: String,
}

impl AuditRecord {
    pub fn new(status: &str) -> Self {
        Self {
            operation_id: Uuid::new_v4().to_string(),
            utc_timestamp: Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            board_model: None,
            board_revision: None,
            chip_identity: None,
            chip_confidence: None,
            programmer: None,
            image_name: None,
            image_size: None,
            image_sha256: None,
            validation_results: vec![],
            backend_args: None,
            backend_exit_code: None,
            pre_write_backup_hash: None,
            candidate_hash: None,
            post_write_hash: None,
            final_status: status.to_string(),
        }
    }

    pub fn save(&self, dir: &Path) -> crate::domain::Result<std::path::PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.json", self.operation_id));
        let f = std::fs::File::create(&path)?;
        serde_json::to_writer_pretty(f, self)?;
        Ok(path)
    }

    pub fn load(path: &Path) -> crate::domain::Result<Self> {
        let f = std::fs::File::open(path)?;
        Ok(serde_json::from_reader(f)?)
    }
}
