//! Board profile loading and image validation against expected geometry.

use crate::domain::{BoardProfile, BoardIdentity, IdentityConfidence, ImageMeta, BiosToolError, Result};
use crate::firmware::InspectReport;

pub fn load_profile(id: &str) -> Result<BoardProfile> {
    // Provisional default profile for Gigabyte B760M AORUS family.
    // Operator must confirm exact SKU before any write.
    if id == "default" || id == "b760m-aorus" {
        Ok(BoardProfile {
            id: id.to_string(),
            identity: BoardIdentity {
                manufacturer: "Gigabyte".into(),
                model: "B760M AORUS (provisional)".into(),
                pcb_revision: "1.1".into(),
                confidence: IdentityConfidence::Assumed,
                notes: Some("Confirm exact retail SKU from PCB label".into()),
            },
            expected_flash_size: 32 * 1024 * 1024,
            expected_chip_family: Some("MX25L256".into()),
        })
    } else {
        Err(BiosToolError::Other(format!("unknown profile: {}", id)))
    }
}

pub fn validate_image_against_profile(
    meta: &ImageMeta,
    report: &InspectReport,
    profile: &BoardProfile,
) -> Result<()> {
    if meta.size != profile.expected_flash_size {
        return Err(BiosToolError::SizeMismatch {
            expected: profile.expected_flash_size,
            actual: meta.size,
        });
    }
    if matches!(meta.kind, crate::domain::ImageKind::Capsule) {
        return Err(BiosToolError::ImageInvalid(
            "capsule cannot be used as full-chip raw image".into(),
        ));
    }
    if matches!(meta.kind, crate::domain::ImageKind::Unknown) {
        return Err(BiosToolError::ImageInvalid("ambiguous image type".into()));
    }
    // Structural notes from inspect are informational
    let _ = report;
    Ok(())
}
