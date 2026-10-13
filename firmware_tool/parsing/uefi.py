"""Conservative, bounded UEFI Firmware Volume parser."""

from __future__ import annotations

import struct
from pathlib import Path
from typing import Optional

from models import Codes, FirmwareRegion, Finding, InspectionReport

# EFI_FVH_SIGNATURE = "_FVH"
FVH_SIG = b"_FVH"
# Minimum header size we need to read safely
MIN_FV_HEADER = 56  # enough for Signature, HeaderLength, etc.


def _safe_u16(data: bytes, offset: int) -> Optional[int]:
    if offset + 2 > len(data):
        return None
    return struct.unpack_from("<H", data, offset)[0]


def _safe_u32(data: bytes, offset: int) -> Optional[int]:
    if offset + 4 > len(data):
        return None
    return struct.unpack_from("<I", data, offset)[0]


def _safe_u64(data: bytes, offset: int) -> Optional[int]:
    if offset + 8 > len(data):
        return None
    return struct.unpack_from("<Q", data, offset)[0]


def find_firmware_volumes(
    data: bytes,
    max_volumes: int = 32,
) -> list[dict]:
    """
    Locate EFI Firmware Volume headers by signature.

    Returns list of {offset, length, header_length, status, warnings}.
    Validates lengths against buffer bounds.
    """
    volumes: list[dict] = []
    search_pos = 0
    data_len = len(data)

    while search_pos < data_len and len(volumes) < max_volumes:
        idx = data.find(FVH_SIG, search_pos)
        if idx == -1:
            break

        # Signature is at offset 40 within the FV header (per PI spec)
        # So the actual header start is idx - 40
        header_start = idx - 40
        if header_start < 0:
            search_pos = idx + 4
            continue

        if header_start + MIN_FV_HEADER > data_len:
            volumes.append(
                {
                    "offset": header_start,
                    "length": None,
                    "header_length": None,
                    "status": "truncated",
                    "warnings": ["Header extends beyond image"],
                }
            )
            search_pos = idx + 4
            continue

        # FvLength is at offset 32 (UINT64)
        fv_length = _safe_u64(data, header_start + 32)
        # HeaderLength is at offset 48 (UINT16)
        header_length = _safe_u16(data, header_start + 48)

        warnings: list[str] = []
        status = "ok"

        if fv_length is None or header_length is None:
            status = "malformed"
            warnings.append("Could not read length fields")
            volumes.append(
                {
                    "offset": header_start,
                    "length": None,
                    "header_length": None,
                    "status": status,
                    "warnings": warnings,
                }
            )
            search_pos = idx + 4
            continue

        # Bounds checks
        if header_length < MIN_FV_HEADER:
            status = "malformed"
            warnings.append(f"HeaderLength too small: {header_length}")
        if fv_length == 0:
            status = "malformed"
            warnings.append("FvLength is zero")
        if header_start + fv_length > data_len:
            status = "out_of_bounds"
            warnings.append(
                f"FvLength {fv_length} exceeds remaining image size"
            )
        # Overflow-safe: use Python int which is unbounded, but still check
        if header_start + fv_length < header_start:
            status = "malformed"
            warnings.append("Integer overflow in offset + length")

        volumes.append(
            {
                "offset": header_start,
                "length": fv_length,
                "header_length": header_length,
                "status": status,
                "warnings": warnings,
            }
        )
        # Advance past this volume if length is sane, else past signature
        if status == "ok" and fv_length > 0:
            search_pos = header_start + fv_length
        else:
            search_pos = idx + 4

    return volumes


def parse_uefi_image(path: Path, report: InspectionReport) -> None:
    """
    Parse a firmware image for UEFI structures and populate the report.

    Mutates report in place (adds regions and findings).
    Never executes any payload.
    """
    try:
        data = path.read_bytes()
    except OSError as exc:
        report.add_finding(
            "error",
            Codes.READ_FAILED,
            f"Cannot read image for UEFI parse: {exc}",
        )
        return

    if len(data) == 0:
        report.add_finding("error", Codes.EMPTY_IMAGE, "Image is empty")
        return

    volumes = find_firmware_volumes(data)

    if not volumes:
        report.add_finding(
            "info",
            Codes.UNKNOWN_IMAGE_TYPE,
            "No EFI Firmware Volume signatures found",
        )
        return

    for i, vol in enumerate(volumes):
        name = f"FV_{i}"
        offset = vol["offset"]
        length = vol["length"] or 0
        status = vol["status"]

        region = FirmwareRegion(
            name=name,
            offset=offset,
            size=length,
            format_name="EFI_FIRMWARE_VOLUME",
            confidence="medium" if status == "ok" else "low",
        )
        report.regions.append(region)

        for w in vol.get("warnings", []):
            report.add_finding(
                "warning" if status != "ok" else "info",
                Codes.PARSE_WARNING,
                f"{name}: {w}",
                offset=offset,
            )

        if status == "malformed":
            report.add_finding(
                "error",
                Codes.MALFORMED_FIRMWARE_VOLUME,
                f"{name} at {offset:#x} has malformed header",
                offset=offset,
            )
        elif status == "out_of_bounds":
            report.add_finding(
                "error",
                Codes.REGION_OUT_OF_BOUNDS,
                f"{name} at {offset:#x} extends beyond image",
                offset=offset,
            )
