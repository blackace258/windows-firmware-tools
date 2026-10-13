//! Exact model/revision compatibility, size/layout validation, image-format checks.

use crate::domain::{BoardProfile, ImageMeta, ImageKind, BiosToolError, Result};
use crate::firmware::InspectionReport;

pub fn validate_image_against_profile(
    image: &ImageMeta,
    report: &InspectionReport,
    profile: &BoardProfile,
) -> Result<()> {
    if image.size != profile.expected_flash_size {
        return Err(BiosToolError::SizeMismatch {
            expected: profile.expected_flash_size,
            actual: image.size,
        });
    }

    if !matches!(image.kind, ImageKind::RawFullChip) && !matches!(report.probable_kind, ImageKind::RawFullChip) {
        return Err(BiosToolError::ImageInvalid(
            "image is not a validated raw full-chip dump".into(),
        ));
    }

    if matches!(report.probable_kind, ImageKind::Capsule) {
        return Err(BiosToolError::ImageInvalid(
            "capsule image must not be used as full-chip write candidate".into(),
        ));
    }

    if !profile.identity.is_write_safe() {
        return Err(BiosToolError::BoardUncertain(
            profile.identity.exact_identity_string(),
        ));
    }

    Ok(())
}

pub fn load_profile(id: &str) -> Result<BoardProfile> {
    // Built-in provisional profile for the stated target. Real deployments should
    // load from verified external profiles.
    if id == "gigabyte-b760m-aorus-rev1.1" || id == "default" {
        Ok(BoardProfile {
            id: id.to_string(),
            identity: crate::domain::BoardIdentity {
                manufacturer: "Gigabyte".into(),
                model: "B760M AORUS".into(), // still provisional until label-confirmed
                pcb_revision: "1.1".into(),
                confidence: crate::domain::IdentityConfidence::Assumed,
                notes: Some("Confirm exact retail SKU from PCB silk/label before write.".into()),
            },
            expected_flash_size: 32 * 1024 * 1024,
            expected_chip_family: Some("MX25L25673G".into()),
        })
    } else {
        Err(BiosToolError::Other(format!("unknown profile id: {}", id)))
    }
}
