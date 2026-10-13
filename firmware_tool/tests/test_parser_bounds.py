"""Tests that parser rejects malformed / out-of-bounds structures."""

from __future__ import annotations

import struct
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from models import InspectionReport, FirmwareArtifact
from parsing.uefi import find_firmware_volumes, parse_uefi_image


def make_minimal_fv(length: int = 0x1000, header_length: int = 0x48) -> bytes:
    """Build a minimal-looking FV header with _FVH signature."""
    # Layout (simplified):
    # 0x00: ZeroVector (16)
    # 0x10: FileSystemGuid (16)
    # 0x20: FvLength (8)
    # 0x28: Signature (4) = "_FVH"
    # 0x2C: Attributes (4)
    # 0x30: HeaderLength (2)
    # 0x32: Checksum (2)
    # ... rest zeroed
    buf = bytearray(length)
    # FvLength at 0x20
    struct.pack_into("<Q", buf, 0x20, length)
    # Signature at 0x28
    buf[0x28:0x2C] = b"_FVH"
    # HeaderLength at 0x30 (spec offset 48 decimal = 0x30)
    # Actually per PI: HeaderLength is at offset 48
    struct.pack_into("<H", buf, 48, header_length)
    return bytes(buf)


class TestParserBounds(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_truncated_header_no_crash(self) -> None:
        # Signature present but not enough bytes for full header
        data = b"\x00" * 30 + b"_FVH" + b"\x00" * 10
        vols = find_firmware_volumes(data)
        # Should not raise; may or may not find a volume
        self.assertIsInstance(vols, list)

    def test_out_of_bounds_length_flagged(self) -> None:
        # Claim a huge length
        data = make_minimal_fv(length=0x100)
        # Overwrite FvLength to something larger than buffer
        data = bytearray(data)
        struct.pack_into("<Q", data, 0x20, 0x1000000)
        data = bytes(data)
        vols = find_firmware_volumes(data)
        self.assertTrue(any(v["status"] == "out_of_bounds" for v in vols) or len(vols) == 0)

    def test_zero_length_flagged(self) -> None:
        # Build a normal-sized buffer but claim FvLength = 0
        data = bytearray(make_minimal_fv(length=0x1000))
        struct.pack_into("<Q", data, 0x20, 0)
        vols = find_firmware_volumes(bytes(data))
        # Zero length should be malformed or skipped
        self.assertTrue(
            any(v["status"] == "malformed" for v in vols) or len(vols) == 0
        )

    def test_parse_empty_file(self) -> None:
        p = self.dir / "empty.bin"
        p.write_bytes(b"")
        art = FirmwareArtifact(path=p, size_bytes=0, sha256="0" * 64, artifact_type="unknown")
        report = InspectionReport(artifact=art)
        parse_uefi_image(p, report)
        codes = [f.code for f in report.findings]
        self.assertIn("EMPTY_IMAGE", codes)

    def test_random_data_no_crash(self) -> None:
        import os
        data = os.urandom(8192)
        vols = find_firmware_volumes(data)
        self.assertIsInstance(vols, list)


if __name__ == "__main__":
    unittest.main()
