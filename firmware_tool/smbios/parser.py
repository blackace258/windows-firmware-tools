"""
Safe SMBIOS structure parser.

Follows the study-course rules:
  - Confirm enough bytes for header
  - Validate Length >= header and within remaining buffer
  - Locate double-NUL string-set terminator within bounds
  - Validate string indexes before resolving
  - Advance only after confirming next offset is in buffer
  - Preserve unknown types
  - Treat all firmware bytes as untrusted
"""

from __future__ import annotations

import struct
from typing import Any, Dict, List, Optional, Tuple

from .models import SmbiosReport, StructureRecord, sha256_bytes
from .acquisition import parse_rsmb_header, AcquisitionError


class ParseError(Exception):
    """Malformed or out-of-bounds SMBIOS data."""


# Minimum formatted lengths for known types (SMBIOS 3.x common fields)
# Length field is the formatted section size including the 4-byte header.
TYPE0_MIN = 0x12  # BIOS Information
TYPE1_MIN = 0x08  # System Information (UUID starts at 0x08 in v2.1+)
TYPE2_MIN = 0x08  # Baseboard
TYPE3_MIN = 0x09  # Enclosure


def _get_string(strings: List[str], index: int) -> Optional[str]:
    """String indexes are 1-based; 0 means no string."""
    if index == 0:
        return None
    if index < 1 or index > len(strings):
        return None  # invalid index — caller may note it
    return strings[index - 1]


def _extract_string_set(data: bytes, start: int) -> Tuple[List[str], int]:
    """
    From start (first byte after formatted section), collect NUL-terminated
    strings until a double-NUL. Returns (strings, end_offset_exclusive).

    Layout: [str1\\0][str2\\0]...[strN\\0]\\0   or   \\0\\0 if no strings.
    """
    strings: List[str] = []
    i = start
    n = len(data)

    if i >= n:
        raise ParseError(f"string-set starts past end of buffer (offset {start})")

    while i < n:
        # End of string-set: two consecutive NUL bytes
        if i + 1 < n and data[i] == 0 and data[i + 1] == 0:
            return strings, i + 2

        # Empty string entry (NUL followed by non-NUL) — rare
        if data[i] == 0:
            strings.append("")
            i += 1
            continue

        # Non-empty string
        j = i
        while j < n and data[j] != 0:
            j += 1
        if j >= n:
            raise ParseError(
                f"string not NUL-terminated before end of buffer (started at {i})"
            )
        try:
            s = data[i:j].decode("utf-8", errors="replace")
        except Exception:
            s = data[i:j].decode("latin-1", errors="replace")
        strings.append(s)
        # j is the NUL that ends this string. If the next byte is also NUL,
        # the string-set terminator is complete (str\\0\\0).
        if j + 1 < n and data[j + 1] == 0:
            return strings, j + 2
        i = j + 1  # more strings follow

    raise ParseError(
        f"string-set double-NUL terminator not found (started at {start})"
    )


def _decode_uuid(raw: bytes) -> str:
    """
    Decode 16-byte UUID per SMBIOS rules for the common case.
    SMBIOS 2.6+ mixed-endian for first three fields; we present canonical
    string form. Do not blindly reverse without checking table version —
    here we use the RFC 4122 style mixed-endian layout common on modern
    Windows tables.
    """
    if len(raw) != 16:
        return raw.hex()
    # SMBIOS UUID: time_low (LE), time_mid (LE), time_hi (LE), then clock_seq
    # and node as stored (big-endian style for last 8 bytes in display)
    t_low = struct.unpack_from("<I", raw, 0)[0]
    t_mid = struct.unpack_from("<H", raw, 4)[0]
    t_hi = struct.unpack_from("<H", raw, 6)[0]
    rest = raw[8:16]
    return f"{t_low:08x}-{t_mid:04x}-{t_hi:04x}-{rest[0]:02x}{rest[1]:02x}-{rest[2:].hex()}"


def _parse_type0(fmt: bytes, strings: List[str]) -> Dict[str, Any]:
    fields: Dict[str, Any] = {}
    if len(fmt) < TYPE0_MIN:
        fields["_truncated"] = True
        return fields
    vendor_idx = fmt[0x04]
    version_idx = fmt[0x05]
    # starting address segment at 0x06-0x07
    release_idx = fmt[0x08]
    fields["vendor"] = _get_string(strings, vendor_idx)
    fields["bios_version"] = _get_string(strings, version_idx)
    fields["bios_release_date"] = _get_string(strings, release_idx)
    if len(fmt) >= 0x14:
        fields["system_bios_major"] = fmt[0x14]
        fields["system_bios_minor"] = fmt[0x15]
    if vendor_idx and _get_string(strings, vendor_idx) is None:
        fields["_note"] = f"vendor string index {vendor_idx} out of range"
    return fields


def _parse_type1(fmt: bytes, strings: List[str]) -> Dict[str, Any]:
    fields: Dict[str, Any] = {}
    if len(fmt) < 0x08:
        fields["_truncated"] = True
        return fields
    fields["manufacturer"] = _get_string(strings, fmt[0x04])
    fields["product_name"] = _get_string(strings, fmt[0x05])
    fields["version"] = _get_string(strings, fmt[0x06])
    fields["serial_number"] = _get_string(strings, fmt[0x07])
    if len(fmt) >= 0x19:
        fields["uuid"] = _decode_uuid(fmt[0x08:0x18])
        fields["wakeup_type"] = fmt[0x18]
    if len(fmt) >= 0x1B:
        fields["sku_number"] = _get_string(strings, fmt[0x19])
        fields["family"] = _get_string(strings, fmt[0x1A])
    return fields


def _parse_type2(fmt: bytes, strings: List[str]) -> Dict[str, Any]:
    fields: Dict[str, Any] = {}
    if len(fmt) < 0x08:
        fields["_truncated"] = True
        return fields
    fields["manufacturer"] = _get_string(strings, fmt[0x04])
    fields["product"] = _get_string(strings, fmt[0x05])
    fields["version"] = _get_string(strings, fmt[0x06])
    fields["serial_number"] = _get_string(strings, fmt[0x07])
    if len(fmt) >= 0x09:
        fields["asset_tag"] = _get_string(strings, fmt[0x08])
    if len(fmt) >= 0x0B:
        fields["location_in_chassis"] = _get_string(strings, fmt[0x0A])
    if len(fmt) >= 0x0D:
        fields["chassis_handle"] = struct.unpack_from("<H", fmt, 0x0B)[0]
    if len(fmt) >= 0x0E:
        fields["board_type"] = fmt[0x0D]
    return fields


def _parse_type3(fmt: bytes, strings: List[str]) -> Dict[str, Any]:
    fields: Dict[str, Any] = {}
    if len(fmt) < TYPE3_MIN:
        fields["_truncated"] = True
        return fields
    fields["manufacturer"] = _get_string(strings, fmt[0x04])
    fields["chassis_type"] = fmt[0x05]
    fields["version"] = _get_string(strings, fmt[0x06])
    fields["serial_number"] = _get_string(strings, fmt[0x07])
    fields["asset_tag"] = _get_string(strings, fmt[0x08])
    return fields


_TYPE_PARSERS = {
    0: _parse_type0,
    1: _parse_type1,
    2: _parse_type2,
    3: _parse_type3,
}


def parse_structures(table_body: bytes) -> Tuple[List[StructureRecord], List[str], List[str]]:
    """
    Walk the SMBIOS table body safely.

    Returns (structures, warnings, errors).
    Does not crash on malformed input; records problems and stops advancing
    when boundaries cannot be validated.
    """
    structures: List[StructureRecord] = []
    warnings: List[str] = []
    errors: List[str] = []
    offset = 0
    n = len(table_body)
    max_structures = 4096  # hard safety bound

    while offset + 4 <= n and len(structures) < max_structures:
        type_ = table_body[offset]
        length = table_body[offset + 1]
        handle = struct.unpack_from("<H", table_body, offset + 2)[0]

        if length < 4:
            errors.append(
                f"offset {offset}: structure Length={length} < 4 (header size); stopping"
            )
            break
        if offset + length > n:
            errors.append(
                f"offset {offset}: structure Length={length} exceeds remaining "
                f"buffer ({n - offset} bytes); stopping"
            )
            break

        formatted = table_body[offset : offset + length]

        # String-set starts immediately after formatted section
        try:
            strings, end_off = _extract_string_set(table_body, offset + length)
        except ParseError as e:
            errors.append(f"offset {offset} type={type_}: {e}; stopping")
            break

        notes: List[str] = []
        fields: Dict[str, Any] = {}
        parser = _TYPE_PARSERS.get(type_)
        if parser:
            fields = parser(formatted, strings)
            # Flag bad string indexes
            for key, val in list(fields.items()):
                if val is None and key not in ("_truncated", "_note"):
                    # None can mean index 0 (valid) or bad index; leave as None
                    pass
        else:
            notes.append(f"unknown or unparsed type {type_}")

        if type_ == 127:
            # End-of-table marker — still record it, then stop
            structures.append(
                StructureRecord(
                    type=type_,
                    length=length,
                    handle=handle,
                    offset=offset,
                    fields={"end_of_table": True},
                    strings=strings,
                    raw_formatted=formatted,
                    notes=notes,
                )
            )
            break

        structures.append(
            StructureRecord(
                type=type_,
                length=length,
                handle=handle,
                offset=offset,
                fields=fields,
                strings=strings,
                raw_formatted=formatted,
                notes=notes,
            )
        )

        # Advance only after validating end_off
        if end_off <= offset:
            errors.append(
                f"offset {offset}: next offset {end_off} did not advance; stopping"
            )
            break
        if end_off > n:
            errors.append(
                f"offset {offset}: next offset {end_off} past buffer end; stopping"
            )
            break
        offset = end_off

    if len(structures) >= max_structures:
        warnings.append(f"stopped after {max_structures} structures (safety bound)")

    return structures, warnings, errors


def parse_smbios_table(
    raw: bytes,
    source: str = "unknown",
    platform_name: str = "Windows",
) -> SmbiosReport:
    """
    Parse a full RSMB buffer (header + table body) into SmbiosReport.
    """
    try:
        major, minor, dmi_rev, table_length, body = parse_rsmb_header(raw)
    except AcquisitionError as e:
        # Allow raw body-only dumps (no 8-byte Windows header) for offline tests
        if len(raw) >= 4 and raw[1] >= 4:
            major, minor, dmi_rev = 0, 0, 0
            table_length = len(raw)
            body = raw
            warnings_extra = [f"no RSMB header; treating entire buffer as table body ({e})"]
        else:
            raise ParseError(str(e)) from e
    else:
        warnings_extra = []

    structures, warnings, errors = parse_structures(body)
    warnings = warnings_extra + warnings

    type0 = type1 = type2 = type3 = None
    for s in structures:
        if s.type == 0 and type0 is None:
            type0 = s.fields
        elif s.type == 1 and type1 is None:
            type1 = s.fields
        elif s.type == 2 and type2 is None:
            type2 = s.fields
        elif s.type == 3 and type3 is None:
            type3 = s.fields

    return SmbiosReport(
        source=source,
        platform=platform_name,
        smbios_major=major,
        smbios_minor=minor,
        dmi_revision=dmi_rev,
        table_length=table_length,
        raw_sha256=sha256_bytes(raw),
        raw_size=len(raw),
        structures=structures,
        type0=type0,
        type1=type1,
        type2=type2,
        type3=type3,
        parse_warnings=warnings,
        parse_errors=errors,
    )
