"""Byte-range binary comparison between two files."""

from __future__ import annotations

from pathlib import Path
from typing import Iterator


def iter_diff_ranges(
    first: Path,
    second: Path,
    chunk_size: int = 1024 * 1024,
    max_ranges: int = 100,
) -> dict:
    """
    Compare two files and report differing byte ranges.

    Returns summary with ranges limited to max_ranges for large images.
    Never modifies either input.
    """
    first_size = first.stat().st_size
    second_size = second.stat().st_size

    if first_size != second_size:
        return {
            "same_size": False,
            "first_size": first_size,
            "second_size": second_size,
            "differing_bytes": None,
            "ranges": [],
            "truncated": True,
            "message": "Files have different sizes; byte comparison limited.",
        }

    ranges: list[dict] = []
    differing_bytes = 0
    current_start: int | None = None
    offset = 0
    truncated = False

    with first.open("rb") as f1, second.open("rb") as f2:
        while True:
            c1 = f1.read(chunk_size)
            c2 = f2.read(chunk_size)
            if not c1 and not c2:
                break
            if len(c1) != len(c2):
                # Should not happen after size check, but be safe
                break

            for i, (b1, b2) in enumerate(zip(c1, c2)):
                abs_off = offset + i
                if b1 != b2:
                    differing_bytes += 1
                    if current_start is None:
                        current_start = abs_off
                else:
                    if current_start is not None:
                        if len(ranges) < max_ranges:
                            ranges.append(
                                {
                                    "start": current_start,
                                    "end": abs_off - 1,
                                    "length": abs_off - current_start,
                                }
                            )
                        else:
                            truncated = True
                        current_start = None
            offset += len(c1)

    # Close any open range
    if current_start is not None:
        if len(ranges) < max_ranges:
            ranges.append(
                {
                    "start": current_start,
                    "end": first_size - 1,
                    "length": first_size - current_start,
                }
            )
        else:
            truncated = True

    return {
        "same_size": True,
        "first_size": first_size,
        "second_size": second_size,
        "differing_bytes": differing_bytes,
        "ranges": ranges,
        "truncated": truncated,
        "identical": differing_bytes == 0,
        "message": (
            "Files are identical"
            if differing_bytes == 0
            else f"{differing_bytes} differing byte(s) found"
        ),
    }
