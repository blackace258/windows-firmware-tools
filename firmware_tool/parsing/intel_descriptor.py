"""Optional Intel Flash Descriptor parsing (conservative)."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from models import Codes, FirmwareRegion, InspectionReport

# Common IFD signature pattern
IFD_SIG = bytes.fromhex("5aa5f00f")


def parse_intel_descriptor(path: Path, report: InspectionReport) -> None:
    """
    Attempt to locate an Intel Flash Descriptor and report regions.

    This is a minimal detector; full region map parsing is left for later.
    Does not assume the format applies to all platforms.
    """
    try:
        data = path.read_bytes()
    except OSError:
        return

    if len(data) < 16:
        return

    # Look near the start
    if data[:4] == IFD_SIG or IFD_SIG in data[:256]:
        report.regions.append(
            FirmwareRegion(
                name="IntelFlashDescriptor",
                offset=0,
                size=min(4096, len(data)),  # descriptor is typically small
                format_name="intel_flash_descriptor",
                confidence="low",
            )
        )
        report.add_finding(
            "info",
            Codes.PARSE_WARNING,
            "Possible Intel Flash Descriptor signature detected near start. "
            "Full region map parsing not yet implemented.",
        )
    # No strong finding if absent — silence is preferred over false positives
