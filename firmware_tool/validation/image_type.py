"""Conservative image type detection."""

from __future__ import annotations

from pathlib import Path
from typing import Optional


# Simple signature markers (conservative)
# EFI Firmware Volume header signature: "_FVH"
FVH_SIGNATURE = b"_FVH"
# UEFI Capsule GUID (EFI_CAPSULE_GUID) first bytes (little-endian)
# 3B6686BD-0D76-4510-87A6-...
CAPSULE_GUID_PREFIX = bytes.fromhex("bd86663b760d104587a6")
# Intel Flash Descriptor signature often starts with 0x5A A5 F0 0F or similar
# Common IFD marker near offset 0 or 0x10
IFD_MARKER = bytes.fromhex("5aa5f00f")


def detect_image_type(path: Path, max_probe: int = 4096) -> dict:
    """
    Conservatively classify an image.

    Returns:
      artifact_type, detected_format, evidence (list), confidence
    """
    evidence: list[str] = []
    confidence = "low"
    artifact_type = "unknown"
    detected_format: Optional[str] = None

    try:
        with path.open("rb") as f:
            header = f.read(max_probe)
    except OSError:
        return {
            "artifact_type": "unknown",
            "detected_format": None,
            "evidence": ["Failed to read file header"],
            "confidence": "none",
        }

    size = path.stat().st_size

    # Check for empty / tiny
    if size == 0:
        return {
            "artifact_type": "unknown",
            "detected_format": None,
            "evidence": ["Empty file"],
            "confidence": "high",
        }

    # Look for EFI Firmware Volume signature
    if FVH_SIGNATURE in header:
        evidence.append("Found EFI Firmware Volume signature (_FVH)")
        artifact_type = "raw_dump"
        detected_format = "uefi_firmware_volume"
        confidence = "medium"

    # Look for capsule-like GUID
    if CAPSULE_GUID_PREFIX in header[:64]:
        evidence.append("Found UEFI Capsule GUID prefix near start")
        if artifact_type == "unknown":
            artifact_type = "vendor_package"
            detected_format = "uefi_capsule_candidate"
        confidence = "medium"

    # Intel Flash Descriptor candidate
    if IFD_MARKER in header[:256] or header[:4] == bytes.fromhex("5aa5f00f"):
        evidence.append("Possible Intel Flash Descriptor marker")
        if artifact_type == "unknown":
            artifact_type = "raw_dump"
            detected_format = "intel_descriptor_candidate"
        confidence = "low"

    # Heuristic: exact common SPI sizes suggest raw dump
    common_sizes = {
        8 * 1024 * 1024,
        16 * 1024 * 1024,
        32 * 1024 * 1024,
        64 * 1024 * 1024,
    }
    if size in common_sizes and artifact_type == "unknown":
        evidence.append(f"Size matches common SPI NOR capacity ({size} bytes)")
        artifact_type = "raw_dump"
        detected_format = "raw_flash_candidate"
        confidence = "low"

    if not evidence:
        evidence.append("No strong signatures found")
        confidence = "none"

    return {
        "artifact_type": artifact_type,
        "detected_format": detected_format,
        "evidence": evidence,
        "confidence": confidence,
    }
