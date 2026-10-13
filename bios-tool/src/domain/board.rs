//! Board identity and profiles. Exact model/revision must be confirmed; never infer from chipset alone.

use serde::{Deserialize, Serialize};

/// Provisional target family. Exact retail SKU must be read from PCB/product label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardIdentity {
    pub manufacturer: String,
    pub model: String,
    pub pcb_revision: String,
    /// Confidence: operator-confirmed vs detected/assumed.
    pub confidence: IdentityConfidence,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IdentityConfidence {
    OperatorConfirmed,
    Detected,
    Assumed,
    Unknown,
}

impl Default for BoardIdentity {
    fn default() -> Self {
        Self {
            manufacturer: "Gigabyte".into(),
            model: "B760M AORUS (provisional)".into(),
            pcb_revision: "1.1".into(),
            confidence: IdentityConfidence::Assumed,
            notes: Some("Exact SKU and flash part must be verified from physical labels before any write.".into()),
        }
    }
}

impl BoardIdentity {
    pub fn is_write_safe(&self) -> bool {
        matches!(self.confidence, IdentityConfidence::OperatorConfirmed)
            && !self.model.contains("provisional")
            && !self.pcb_revision.is_empty()
    }

    pub fn exact_identity_string(&self) -> String {
        format!("{} {} PCB:{}", self.manufacturer, self.model, self.pcb_revision)
    }
}

/// Board profile used for validation (geometry expectations, known good layouts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardProfile {
    pub id: String,
    pub identity: BoardIdentity,
    pub expected_flash_size: u64,
    pub expected_chip_family: Option<String>,
}
