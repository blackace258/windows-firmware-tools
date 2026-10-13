"""Abstract read-only acquisition interface."""

from __future__ import annotations

from abc import ABC, abstractmethod
from pathlib import Path
from typing import Optional

from models import FirmwareArtifact, FlashChip


class AcquisitionSource(ABC):
    """Abstract base for read-only firmware acquisition."""

    @abstractmethod
    def acquire(self) -> FirmwareArtifact:
        """Acquire the firmware artifact without modifying source."""
        ...

    @abstractmethod
    def identify_chip(self) -> Optional[FlashChip]:
        """Attempt chip identification. Return None if unknown."""
        ...


class FileAcquisitionError(Exception):
    """Raised for file acquisition failures."""

    def __init__(self, code: str, message: str):
        self.code = code
        self.message = message
        super().__init__(message)
