"""Load an existing binary firmware file (read-only)."""

from __future__ import annotations

import hashlib
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from acquisition.interface import AcquisitionSource, FileAcquisitionError
from models import Codes, FirmwareArtifact, FlashChip


class FileSource(AcquisitionSource):
    """Read-only acquisition from a local binary file."""

    def __init__(self, path: Path, allow_empty: bool = False):
        self.path = Path(path)
        self.allow_empty = allow_empty
        self._size: Optional[int] = None
        self._sha256: Optional[str] = None

    def acquire(self) -> FirmwareArtifact:
        if not self.path.is_file():
            raise FileAcquisitionError(
                Codes.FILE_NOT_FOUND,
                f"File not found or not a regular file: {self.path}",
            )

        digest = hashlib.sha256()
        size = 0
        chunk_size = 1024 * 1024  # 1 MiB

        try:
            with self.path.open("rb") as stream:
                while True:
                    chunk = stream.read(chunk_size)
                    if not chunk:
                        break
                    size += len(chunk)
                    digest.update(chunk)
        except OSError as exc:
            raise FileAcquisitionError(
                Codes.READ_FAILED,
                f"Failed to read file: {exc}",
            ) from exc

        if size == 0 and not self.allow_empty:
            raise FileAcquisitionError(
                Codes.EMPTY_IMAGE,
                f"File is empty: {self.path}",
            )

        self._size = size
        self._sha256 = digest.hexdigest()

        timestamp = datetime.now(timezone.utc).isoformat()

        return FirmwareArtifact(
            path=self.path.resolve(),
            size_bytes=size,
            sha256=self._sha256,
            artifact_type="unknown",  # refined later by image_type detector
            detected_format=None,
            timestamp_utc=timestamp,
        )

    def identify_chip(self) -> Optional[FlashChip]:
        # File source cannot identify the physical chip.
        return None

    @property
    def size_bytes(self) -> Optional[int]:
        return self._size

    @property
    def sha256(self) -> Optional[str]:
        return self._sha256
