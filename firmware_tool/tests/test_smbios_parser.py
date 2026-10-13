"""Unit tests for safe SMBIOS parser (stdlib only, offline)."""

from __future__ import annotations

import struct
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from smbios.parser import parse_structures, parse_smbios_table, ParseError
from smbios.acquisition import parse_rsmb_header
from smbios.compare import compare_reports


def _build_structure(type_: int, handle: int, formatted_payload: bytes, strings: list[str]) -> bytes:
    """Build one structure: header + formatted (without header) + string-set."""
    # formatted section = 4-byte header + payload
    length = 4 + len(formatted_payload)
    header = bytes([type_, length & 0xFF]) + struct.pack("<H", handle)
    body = header + formatted_payload
    str_blob = b""
    for s in strings:
        str_blob += s.encode("utf-8") + b"\x00"
    str_blob += b"\x00"  # double-NUL terminator (extra NUL after last string)
    if not strings:
        str_blob = b"\x00\x00"
    return body + str_blob


def make_synthetic_rsmb() -> bytes:
    """Minimal Type 0/1/2/3 + end-of-table for offline tests."""
    # Type 0: vendor, version, date indexes 1,2,3; starting addr 0x0000; release at 0x08
    # formatted payload after header: vendor_idx, ver_idx, start_seg(2), date_idx, rom_size, characteristics(8)
    t0_payload = bytes([
        1,  # vendor
        2,  # version
        0x00, 0xE8,  # starting address segment
        3,  # release date
        0x0F,  # ROM size
    ]) + b"\x00" * 8  # characteristics placeholder → total formatted len = 4+6+8=18 = 0x12
    t0 = _build_structure(0, 0x0000, t0_payload, ["TestBIOS Vendor", "1.0.0", "01/01/2024"])

    # Type 1: manufacturer, product, version, serial; UUID 16 bytes; wakeup; sku; family
    uuid = bytes([
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
    ])
    t1_payload = bytes([1, 2, 3, 4]) + uuid + bytes([0x06, 5, 6])
    t1 = _build_structure(
        1, 0x0001, t1_payload,
        ["TestMfg", "TestProduct", "1.0", "SN-ABC-001", "SKU-1", "FamilyX"],
    )

    # Type 2
    t2_payload = bytes([1, 2, 3, 4, 5, 0, 0x00, 0x03, 0x0A])  # chassis handle 0x0003, board type
    t2 = _build_structure(
        2, 0x0002, t2_payload,
        ["BoardMfg", "BoardProduct", "Rev1.1", "BSN-99", "ATAG"],
    )

    # Type 3
    t3_payload = bytes([1, 0x03, 2, 3, 4])  # chassis type desktop-ish
    t3 = _build_structure(
        3, 0x0003, t3_payload,
        ["ChassisMfg", "CVer", "CSN-1", "CATAG"],
    )

    # End of table
    eot = _build_structure(127, 0xFFFF, b"", [])

    table_body = t0 + t1 + t2 + t3 + eot
    # RSMB header
    header = bytes([0, 3, 5, 0]) + struct.pack("<I", len(table_body))
    return header + table_body


class TestSmbiosParser(unittest.TestCase):
    def test_synthetic_table_parses(self):
        raw = make_synthetic_rsmb()
        report = parse_smbios_table(raw, source="test:synthetic")
        self.assertEqual(report.smbios_major, 3)
        self.assertEqual(report.smbios_minor, 5)
        self.assertTrue(report.type0)
        self.assertEqual(report.type0.get("vendor"), "TestBIOS Vendor")
        self.assertEqual(report.type1.get("serial_number"), "SN-ABC-001")
        self.assertEqual(report.type1.get("product_name"), "TestProduct")
        self.assertEqual(report.type2.get("serial_number"), "BSN-99")
        self.assertEqual(report.type3.get("serial_number"), "CSN-1")
        self.assertFalse(report.parse_errors)

    def test_rsmb_header_bounds(self):
        raw = make_synthetic_rsmb()
        major, minor, dmi, length, body = parse_rsmb_header(raw)
        self.assertEqual(major, 3)
        self.assertEqual(length, len(body))

    def test_malformed_length_stops(self):
        # Length claims more than remaining buffer
        bad = bytes([1, 0xFF, 0x00, 0x00]) + b"\x00" * 4
        structs, warnings, errors = parse_structures(bad)
        self.assertTrue(errors)

    def test_missing_double_nul_stops(self):
        # Formatted ok but no string terminator
        bad = bytes([0, 0x12, 0x00, 0x00]) + b"\x00" * 14 + b"NO_TERM"
        structs, warnings, errors = parse_structures(bad)
        self.assertTrue(errors)

    def test_compare_identical(self):
        raw = make_synthetic_rsmb()
        a = parse_smbios_table(raw, source="a")
        b = parse_smbios_table(raw, source="b")
        cmp = compare_reports(a, b)
        self.assertTrue(cmp["identical_raw"])
        self.assertEqual(cmp["difference_count"], 0)

    def test_compare_different_serial(self):
        raw = make_synthetic_rsmb()
        a = parse_smbios_table(raw, source="a")
        b = parse_smbios_table(raw, source="b")
        # Field-level difference only (same raw hash); compare still reports it
        if b.type1:
            b.type1 = dict(b.type1)
            b.type1["serial_number"] = "CHANGED"
        cmp = compare_reports(a, b)
        self.assertTrue(any("serial_number" in d for d in cmp["differences"]))
        self.assertGreaterEqual(cmp["difference_count"], 1)


if __name__ == "__main__":
    unittest.main()
