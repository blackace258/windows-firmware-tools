//! Read-only parsers for raw flash, Intel flash descriptor, firmware volumes,
//! capsules, PE/COFF payloads, and plausible variable-store structures.
//! Treat all images as untrusted. Checked offsets, size limits, no panics on malformed.

pub mod descriptor;
pub mod uefi;
pub mod signatures;

use crate::domain::{ImageKind, Result, BiosToolError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionReport {
    pub size: u64,
    pub sha256: String,
    pub probable_kind: ImageKind,
    pub signatures_found: Vec<String>,
    pub structures_validated: Vec<String>,
    pub notes: Vec<String>,
    pub errors: Vec<String>,
}

pub fn inspect_image(data: &[u8]) -> InspectionReport {
    let size = data.len() as u64;
    let sha256 = crate::domain::ImageMeta::hash_bytes(data);
    let mut signatures = Vec::new();
    let mut validated = Vec::new();
    let mut notes = Vec::new();
    let mut errors = Vec::new();

    // Signature presence (not proof of validity)
    if let Some(s) = signatures::find_signatures(data) {
        signatures.extend(s);
    }

    // Bounded descriptor parse
    match descriptor::try_parse_intel_descriptor(data) {
        Ok(Some(desc)) => {
            validated.push(format!("Intel Flash Descriptor (FLVALSIG ok, regions: {})", desc.region_count));
            notes.push(format!("descriptor base size hint: {}", desc.total_size_hint.unwrap_or(0)));
        }
        Ok(None) => notes.push("no Intel flash descriptor signature found".into()),
        Err(e) => errors.push(format!("descriptor parse: {}", e)),
    }

    // Bounded UEFI FV
    match uefi::find_firmware_volumes(data) {
        Ok(vols) => {
            for (i, v) in vols.iter().enumerate() {
                validated.push(format!("UEFI FV[{}] offset=0x{:x} size=0x{:x}", i, v.offset, v.size));
            }
        }
        Err(e) => errors.push(format!("FV scan: {}", e)),
    }

    let probable_kind = if size == 32 * 1024 * 1024 && signatures.iter().any(|s| s.contains("MX25") || s.contains("Flash")) {
        ImageKind::RawFullChip
    } else if signatures.iter().any(|s| s.contains("Capsule") || s.contains("EFI_CAPSULE")) {
        ImageKind::Capsule
    } else if !validated.is_empty() {
        ImageKind::RawFullChip
    } else {
        ImageKind::Unknown
    };

    InspectionReport {
        size,
        sha256,
        probable_kind,
        signatures_found: signatures,
        structures_validated: validated,
        notes,
        errors,
    }
}

pub fn load_and_inspect(path: &std::path::Path) -> Result<InspectionReport> {
    let data = std::fs::read(path)?;
    if data.len() > 128 * 1024 * 1024 {
        return Err(BiosToolError::Parse("image exceeds 128 MiB safety limit".into()));
    }
    Ok(inspect_image(&data))
}
