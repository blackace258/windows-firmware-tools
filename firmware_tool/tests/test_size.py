"""Unit tests for size / capacity checks."""

from __future__ import annotations

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from validation.size import (
    bits_to_bytes,
    check_size_against_capacity,
    suggest_common_capacity,
)


class TestSize(unittest.TestCase):
    def test_bits_to_bytes(self) -> None:
        self.assertEqual(bits_to_bytes(64 * 1024 * 1024), 8 * 1024 * 1024)
        self.assertEqual(bits_to_bytes(128 * 1024 * 1024), 16 * 1024 * 1024)

    def test_match(self) -> None:
        r = check_size_against_capacity(16 * 1024 * 1024, 16 * 1024 * 1024)
        self.assertTrue(r["match"])
        self.assertEqual(r["status"], "match")

    def test_mismatch(self) -> None:
        r = check_size_against_capacity(16 * 1024 * 1024, 32 * 1024 * 1024)
        self.assertFalse(r["match"])
        self.assertEqual(r["status"], "mismatch")

    def test_unknown(self) -> None:
        r = check_size_against_capacity(16 * 1024 * 1024, None)
        self.assertIsNone(r["match"])
        self.assertEqual(r["status"], "unknown")

    def test_suggest(self) -> None:
        self.assertEqual(suggest_common_capacity(16 * 1024 * 1024), 16 * 1024 * 1024)
        self.assertIsNone(suggest_common_capacity(12345))


if __name__ == "__main__":
    unittest.main()
