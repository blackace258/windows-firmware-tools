"""File size and expected-capacity checks."""

from __future__ import annotations

from typing import Optional

# Common SPI NOR capacities (bits -> bytes)
COMMON_CAPACITIES_BITS = {
    64: 8 * 1024 * 1024,      # 64 Mbit = 8 MiB
    128: 16 * 1024 * 1024,    # 128 Mbit = 16 MiB
    256: 32 * 1024 * 1024,    # 256 Mbit = 32 MiB
    512: 64 * 1024 * 1024,    # 512 Mbit = 64 MiB
    1024: 128 * 1024 * 1024,  # 1 Gbit = 128 MiB
}


def bits_to_bytes(bits: int) -> int:
    """Convert capacity in bits to bytes using integer arithmetic."""
    if bits % 8 != 0:
        raise ValueError(f"Capacity in bits must be divisible by 8: {bits}")
    return bits // 8


def check_size_against_capacity(
    file_size_bytes: int,
    expected_capacity_bytes: Optional[int] = None,
) -> dict:
    """
    Report size relationship to expected chip capacity.

    Does NOT resize, pad, or truncate the image.
    """
    result = {
        "file_size_bytes": file_size_bytes,
        "expected_capacity_bytes": expected_capacity_bytes,
        "match": None,
        "status": "unknown",
        "message": "",
    }

    if expected_capacity_bytes is None:
        result["status"] = "unknown"
        result["message"] = (
            "No independent chip capacity provided; size check inconclusive."
        )
        return result

    if file_size_bytes == expected_capacity_bytes:
        result["match"] = True
        result["status"] = "match"
        result["message"] = (
            f"File size ({file_size_bytes} bytes) matches expected capacity."
        )
    else:
        result["match"] = False
        result["status"] = "mismatch"
        result["message"] = (
            f"File size ({file_size_bytes} bytes) does not match "
            f"expected capacity ({expected_capacity_bytes} bytes)."
        )

    return result


def suggest_common_capacity(file_size_bytes: int) -> Optional[int]:
    """Return a common capacity if the size exactly matches one, else None."""
    for bits, capacity in COMMON_CAPACITIES_BITS.items():
        if capacity == file_size_bytes:
            return capacity
    return None
