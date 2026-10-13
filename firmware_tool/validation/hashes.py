"""SHA-256 hashing and repeat-read comparison utilities."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Optional


def sha256_file(path: Path, chunk_size: int = 1024 * 1024) -> str:
    """Compute SHA-256 of a file. Does not modify the file."""
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(chunk_size), b""):
            digest.update(chunk)
    return digest.hexdigest()


def compare_hashes(first: Path, second: Path) -> dict:
    """
    Compare two files by size and SHA-256.

    Returns a dict with:
      same_size, first_sha256, second_sha256, identical
    """
    first_size = first.stat().st_size
    second_size = second.stat().st_size
    first_hash = sha256_file(first)
    second_hash = sha256_file(second)

    return {
        "same_size": first_size == second_size,
        "first_size": first_size,
        "second_size": second_size,
        "first_sha256": first_hash,
        "second_sha256": second_hash,
        "identical": first_size == second_size and first_hash == second_hash,
    }
