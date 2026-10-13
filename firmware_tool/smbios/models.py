"""Data models for SMBIOS structures and reports."""

from __future__ import annotations

from dataclasses import dataclass, field, asdict
from typing import Any, Dict, List, Optional
import hashlib


@dataclass
class StructureRecord:
    """One SMBIOS structure after safe parsing."""

    type: int
    length: int
    handle: int
    offset: int  # byte offset of structure header in table body
    fields: Dict[str, Any] = field(default_factory=dict)
    strings: List[str] = field(default_factory=list)
    raw_formatted: bytes = b""
    notes: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        d = {
            "type": self.type,
            "length": self.length,
            "handle": self.handle,
            "offset": self.offset,
            "fields": self.fields,
            "strings": self.strings,
            "notes": self.notes,
        }
        return d


@dataclass
class SmbiosReport:
    """Full report for one SMBIOS capture."""

    # Provenance
    source: str  # "GetSystemFirmwareTable(RSMB)" | "file:<path>"
    platform: str  # e.g. "Windows"
    smbios_major: int
    smbios_minor: int
    dmi_revision: int
    table_length: int
    raw_sha256: str
    raw_size: int

    structures: List[StructureRecord] = field(default_factory=list)
    type0: Optional[Dict[str, Any]] = None
    type1: Optional[Dict[str, Any]] = None
    type2: Optional[Dict[str, Any]] = None
    type3: Optional[Dict[str, Any]] = None

    parse_warnings: List[str] = field(default_factory=list)
    parse_errors: List[str] = field(default_factory=list)

    # Explicit layer note — published table only
    layer_note: str = (
        "Values below are from the OS-published SMBIOS table only. "
        "They do not prove the persistent firmware storage source or that "
        "a write path exists for any field."
    )

    def to_dict(self) -> Dict[str, Any]:
        return {
            "source": self.source,
            "platform": self.platform,
            "smbios_version": f"{self.smbios_major}.{self.smbios_minor}",
            "smbios_major": self.smbios_major,
            "smbios_minor": self.smbios_minor,
            "dmi_revision": self.dmi_revision,
            "table_length": self.table_length,
            "raw_size": self.raw_size,
            "raw_sha256": self.raw_sha256,
            "layer_note": self.layer_note,
            "type0_bios": self.type0,
            "type1_system": self.type1,
            "type2_baseboard": self.type2,
            "type3_enclosure": self.type3,
            "structure_count": len(self.structures),
            "structures": [s.to_dict() for s in self.structures],
            "parse_warnings": self.parse_warnings,
            "parse_errors": self.parse_errors,
        }


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()
