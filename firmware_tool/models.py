"""Typed data models for firmware inspection."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class FlashChip:
    manufacturer: Optional[str] = None
    part_number: Optional[str] = None
    capacity_bytes: Optional[int] = None
    voltage_requirements: Optional[str] = None
    identification_source: str = "not identified"


@dataclass
class FirmwareArtifact:
    path: Path
    size_bytes: int
    sha256: str
    artifact_type: str  # "raw_dump", "vendor_package", "unknown"
    detected_format: Optional[str] = None
    timestamp_utc: Optional[str] = None


@dataclass
class FirmwareRegion:
    name: str
    offset: int
    size: int
    format_name: Optional[str] = None
    guid: Optional[str] = None
    parent: Optional[str] = None
    confidence: str = "unknown"  # "high", "medium", "low", "unknown"


@dataclass
class Finding:
    severity: str  # "info", "warning", "error"
    code: str
    message: str
    offset: Optional[int] = None


@dataclass
class ValidationResult:
    size_check: str = "unknown"
    repeat_read_check: str = "not performed"
    compatibility: str = "not established"
    hash_match: Optional[bool] = None


@dataclass
class InspectionReport:
    artifact: FirmwareArtifact
    chip: Optional[FlashChip] = None
    validation: ValidationResult = field(default_factory=ValidationResult)
    regions: list[FirmwareRegion] = field(default_factory=list)
    findings: list[Finding] = field(default_factory=list)

    def add_finding(
        self,
        severity: str,
        code: str,
        message: str,
        offset: Optional[int] = None,
    ) -> None:
        self.findings.append(
            Finding(severity=severity, code=code, message=message, offset=offset)
        )


# Stable error / finding codes
class Codes:
    FILE_NOT_FOUND = "FILE_NOT_FOUND"
    EMPTY_IMAGE = "EMPTY_IMAGE"
    CHIP_NOT_IDENTIFIED = "CHIP_NOT_IDENTIFIED"
    CAPACITY_MISMATCH = "CAPACITY_MISMATCH"
    UNSUPPORTED_PROGRAMMER = "UNSUPPORTED_PROGRAMMER"
    READ_FAILED = "READ_FAILED"
    REPEAT_READ_MISMATCH = "REPEAT_READ_MISMATCH"
    MALFORMED_FIRMWARE_VOLUME = "MALFORMED_FIRMWARE_VOLUME"
    REGION_OUT_OF_BOUNDS = "REGION_OUT_OF_BOUNDS"
    UNKNOWN_IMAGE_TYPE = "UNKNOWN_IMAGE_TYPE"
    COMPATIBILITY_NOT_ESTABLISHED = "COMPATIBILITY_NOT_ESTABLISHED"
    HASH_COMPUTED = "HASH_COMPUTED"
    SIZE_REPORTED = "SIZE_REPORTED"
    PARSE_WARNING = "PARSE_WARNING"
    IDENTICAL_DUMPS = "IDENTICAL_DUMPS"
    DIFFERENT_DUMPS = "DIFFERENT_DUMPS"
    INCONCLUSIVE_COMPARE = "INCONCLUSIVE_COMPARE"
