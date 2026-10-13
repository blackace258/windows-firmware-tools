"""Gates for the SMBIOS write coordinator."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from smbios.write import (
    FieldUpdate,
    WriteBlocked,
    WriteRequest,
    WriteState,
    run_write_workflow,
)


def _request(backup: Path, *, dry_run: bool = True) -> WriteRequest:
    return WriteRequest(
        board_model="Gigabyte B760M AORUS",
        board_rev="1.1",
        exact_identity="Gigabyte B760M AORUS PCB:1.1",
        updates=[FieldUpdate("type1.serial_number", "ABC")],
        backup_raw_path=backup,
        recovery_plan="external programmer plus known-good dump",
        confirm_phrase="I AUTHORIZE DMI WRITE ON Gigabyte B760M AORUS PCB:1.1",
        dry_run=dry_run,
    )


class TestSmbiosWriteGates(unittest.TestCase):
    def test_dry_run_is_not_verified(self):
        with tempfile.TemporaryDirectory() as tmp:
            backup = Path(tmp) / "backup.bin"
            backup.write_bytes(b"RSMBFAKE")
            audit = run_write_workflow(_request(backup))
        self.assertTrue(audit.dry_run)
        self.assertEqual(audit.state, WriteState.WRITE_AUTHORIZED.value)
        self.assertNotEqual(audit.state, WriteState.VERIFIED.value)

    def test_bad_phrase_blocks(self):
        with tempfile.TemporaryDirectory() as tmp:
            backup = Path(tmp) / "backup.bin"
            backup.write_bytes(b"RSMBFAKE")
            req = _request(backup)
            req.confirm_phrase = "yes"
            with self.assertRaises(WriteBlocked):
                run_write_workflow(req)


if __name__ == "__main__":
    unittest.main()
