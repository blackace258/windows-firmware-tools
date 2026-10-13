"""Board/chip/layout compatibility checks (conservative)."""

from __future__ import annotations

from typing import Optional

from models import FlashChip, FirmwareArtifact


def check_compatibility(
    artifact: FirmwareArtifact,
    chip: Optional[FlashChip] = None,
) -> dict:
    """
    Perform conservative compatibility assessment.

    Never claims compatibility without strong evidence.
    """
    result = {
        "status": "not established",
        "reasons": [],
        "can_proceed": False,
    }

    if chip is None or chip.identification_source == "not identified":
        result["reasons"].append("Chip identity unknown")
        result["status"] = "not established"
        return result

    if chip.capacity_bytes is None:
        result["reasons"].append("Chip capacity unknown")
        return result

    if artifact.size_bytes != chip.capacity_bytes:
        result["reasons"].append(
            f"Image size ({artifact.size_bytes}) != chip capacity "
            f"({chip.capacity_bytes})"
        )
        result["status"] = "mismatch"
        return result

    # Size matches and chip known — still not full proof of board match
    result["reasons"].append(
        "Size matches known chip capacity; board identity still required"
    )
    result["status"] = "size_matches_chip"
    # Still not enough for write operations
    result["can_proceed"] = False
    return result
