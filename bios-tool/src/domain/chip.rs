//! Flash chip geometry and identification. Capacity must come from probe, not assumption.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChipIdentity {
    pub vendor: String,
    pub part_number: String,
    pub capacity_bytes: u64,
    pub voltage_class: Option<String>,
    pub confidence: ChipConfidence,
    pub probe_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChipConfidence {
    Probed,
    OperatorSupplied,
    Assumed,
    Unknown,
}

impl Default for ChipIdentity {
    fn default() -> Self {
        // MX25L25673G family is 256 Mbit = 32 MiB, 3-V-class, but treat as provisional.
        Self {
            vendor: "Macronix".into(),
            part_number: "MX25L25673G (provisional)".into(),
            capacity_bytes: 32 * 1024 * 1024,
            voltage_class: Some("3V".into()),
            confidence: ChipConfidence::Assumed,
            probe_raw: None,
        }
    }
}

impl ChipIdentity {
    pub fn is_write_safe(&self) -> bool {
        matches!(self.confidence, ChipConfidence::Probed | ChipConfidence::OperatorSupplied)
            && self.capacity_bytes > 0
            && !self.part_number.contains("provisional")
    }

    pub fn size_mib(&self) -> f64 {
        self.capacity_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChipGeometry {
    pub capacity: u64,
    pub page_size: Option<u64>,
    pub sector_size: Option<u64>,
    pub block_size: Option<u64>,
}
