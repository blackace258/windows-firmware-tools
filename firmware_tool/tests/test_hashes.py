"""Unit tests for hashing and comparison."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from validation.hashes import compare_hashes, sha256_file
from comparison.binary_diff import iter_diff_ranges


class TestHashes(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _write(self, name: str, data: bytes) -> Path:
        p = self.dir / name
        p.write_bytes(data)
        return p

    def test_sha256_stable(self) -> None:
        data = b"firmware-test-data-12345"
        p = self._write("a.bin", data)
        h1 = sha256_file(p)
        h2 = sha256_file(p)
        self.assertEqual(h1, h2)
        expected = hashlib.sha256(data).hexdigest()
        self.assertEqual(h1, expected)

    def test_identical_files(self) -> None:
        data = b"\x00" * 4096 + b"\xff" * 256
        a = self._write("a.bin", data)
        b = self._write("b.bin", data)
        result = compare_hashes(a, b)
        self.assertTrue(result["identical"])
        self.assertTrue(result["same_size"])

    def test_one_byte_difference(self) -> None:
        data = bytearray(b"\x00" * 1024)
        a = self._write("a.bin", bytes(data))
        data[500] = 0x42
        b = self._write("b.bin", bytes(data))
        result = compare_hashes(a, b)
        self.assertFalse(result["identical"])
        self.assertTrue(result["same_size"])

        ranges = iter_diff_ranges(a, b)
        self.assertFalse(ranges["identical"])
        self.assertEqual(ranges["differing_bytes"], 1)
        self.assertEqual(len(ranges["ranges"]), 1)
        self.assertEqual(ranges["ranges"][0]["start"], 500)

    def test_different_sizes(self) -> None:
        a = self._write("a.bin", b"\x00" * 100)
        b = self._write("b.bin", b"\x00" * 200)
        result = compare_hashes(a, b)
        self.assertFalse(result["identical"])
        self.assertFalse(result["same_size"])


if __name__ == "__main__":
    unittest.main()
