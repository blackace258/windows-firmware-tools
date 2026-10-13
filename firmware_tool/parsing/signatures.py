"""Conservative signature identification helpers."""

from __future__ import annotations

from pathlib import Path


KNOWN_SIGNATURES = {
    b"_FVH": "EFI Firmware Volume Header",
    b"MZ": "PE/COFF executable (possible embedded)",
    bytes.fromhex("5aa5f00f"): "Possible Intel Flash Descriptor",
}


def scan_signatures(path: Path, max_bytes: int = 65536) -> list[dict]:
    """
    Scan the first max_bytes for known signatures.
    Returns list of {offset, signature_name, hex}.
    """
    results: list[dict] = []
    try:
        with path.open("rb") as f:
            data = f.read(max_bytes)
    except OSError:
        return results

    for sig, name in KNOWN_SIGNATURES.items():
        start = 0
        while True:
            idx = data.find(sig, start)
            if idx == -1:
                break
            results.append(
                {
                    "offset": idx,
                    "name": name,
                    "hex": sig.hex(),
                }
            )
            start = idx + 1
            if len(results) > 50:  # safety limit
                return results
    return results
