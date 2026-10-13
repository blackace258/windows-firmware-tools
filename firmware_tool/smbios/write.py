"""
Gated DMI/SMBIOS write workflow (Python mirror of smbios-tool write module).

Safety state machine (Lesson 11):
  Unidentified → Identified → BackupVerified → InputValidated
  → RecoveryConfirmed → WriteAuthorized → Executing → Verified
  Any failure → Blocked

Fail-closed. Real execution only invokes an operator-supplied vendor utility.
Never patches SPI images or the published table in process memory.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import uuid
from dataclasses import dataclass, field, asdict
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional


CONFIRM_PHRASE_PREFIX = "I AUTHORIZE DMI WRITE ON"

ALLOWED_FIELDS = frozenset(
    {
        "type1.serial_number",
        "type1.uuid",
        "type1.product_name",
        "type1.manufacturer",
        "type2.serial_number",
        "type2.product",
        "type2.manufacturer",
        "type3.serial_number",
        "type3.asset_tag",
    }
)


class WriteState(str, Enum):
    UNIDENTIFIED = "Unidentified"
    IDENTIFIED = "Identified"
    BACKUP_VERIFIED = "BackupVerified"
    INPUT_VALIDATED = "InputValidated"
    RECOVERY_CONFIRMED = "RecoveryConfirmed"
    WRITE_AUTHORIZED = "WriteAuthorized"
    EXECUTING = "Executing"
    VERIFIED = "Verified"
    BLOCKED = "Blocked"


class WriteBlocked(Exception):
    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"write blocked by safety gate: {reason}")


@dataclass
class FieldUpdate:
    field: str
    new_value: str


@dataclass
class WriteRequest:
    board_model: str
    board_rev: str
    exact_identity: str
    updates: List[FieldUpdate]
    backup_raw_path: Path
    recovery_plan: str
    confirm_phrase: str
    dry_run: bool = True
    vendor_tool_path: Optional[Path] = None
    vendor_tool_args: List[str] = field(default_factory=list)
    vendor_tool_sha256: Optional[str] = None


@dataclass
class WriteAudit:
    operation_id: str
    state: str
    request_summary: str
    backup_sha256: Optional[str]
    vendor_tool: Optional[str]
    dry_run: bool
    messages: List[str]
    exit_code: Optional[int]
    timestamp: str

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class WriteCoordinator:
    def __init__(self, dry_run: bool = True) -> None:
        self.state = WriteState.UNIDENTIFIED
        self.dry_run = dry_run
        self.messages: List[str] = []
        self.backup_sha256: Optional[str] = None
        self.request: Optional[WriteRequest] = None

    def _block(self, reason: str) -> None:
        self.state = WriteState.BLOCKED
        self.messages.append(f"BLOCKED: {reason}")
        raise WriteBlocked(reason)

    def _require(self, *allowed: WriteState) -> None:
        if self.state not in allowed:
            self._block(f"invalid state {self.state.value} for this transition")

    def identify(self, board_model: str, board_rev: str, exact_identity: str) -> None:
        if self.state != WriteState.UNIDENTIFIED:
            self._block(
                f"identify is only valid from Unidentified (current {self.state.value})"
            )
        if not board_model.strip() or not board_rev.strip():
            self._block("board model and PCB revision are required")
        if not exact_identity.strip():
            self._block("exact_identity is required")
        self.request = WriteRequest(
            board_model=board_model,
            board_rev=board_rev,
            exact_identity=exact_identity,
            updates=[],
            backup_raw_path=Path(),
            recovery_plan="",
            confirm_phrase="",
            dry_run=self.dry_run,
        )
        self.messages.append(f"identified board: {exact_identity}")
        self.state = WriteState.IDENTIFIED

    def verify_backup(self, backup_path: Path) -> None:
        self._require(WriteState.IDENTIFIED)
        backup_path = Path(backup_path)
        if not backup_path.is_file():
            self._block(f"backup file not found: {backup_path}")
        data = backup_path.read_bytes()
        if len(data) < 8:
            self._block("backup too small to be a valid RSMB capture")
        self.backup_sha256 = hashlib.sha256(data).hexdigest()
        assert self.request is not None
        self.request.backup_raw_path = backup_path
        self.messages.append(
            f"backup verified: {backup_path} sha256={self.backup_sha256}"
        )
        self.state = WriteState.BACKUP_VERIFIED

    def validate_input(self, updates: List[FieldUpdate]) -> None:
        self._require(WriteState.BACKUP_VERIFIED)
        if not updates:
            self._block("no field updates specified")
        for u in updates:
            if u.field not in ALLOWED_FIELDS:
                self._block(
                    f"field '{u.field}' is not in the allowed write list: "
                    f"{sorted(ALLOWED_FIELDS)}"
                )
            if not u.new_value:
                self._block(f"empty new_value for field {u.field}")
            if len(u.new_value) > 64:
                self._block(f"new_value for {u.field} exceeds 64 characters")
        assert self.request is not None
        self.request.updates = updates
        self.messages.append(f"input validated: {len(updates)} field(s)")
        self.state = WriteState.INPUT_VALIDATED

    def confirm_recovery(self, plan: str) -> None:
        self._require(WriteState.INPUT_VALIDATED)
        plan = plan.strip()
        if len(plan) < 16:
            self._block(
                "recovery_plan must describe a concrete method (≥16 chars)"
            )
        assert self.request is not None
        self.request.recovery_plan = plan
        self.messages.append(f"recovery plan recorded: {plan}")
        self.state = WriteState.RECOVERY_CONFIRMED

    def authorize(self, phrase: str) -> None:
        self._require(WriteState.RECOVERY_CONFIRMED)
        assert self.request is not None
        expected = f"{CONFIRM_PHRASE_PREFIX} {self.request.exact_identity}"
        if phrase.strip() != expected:
            self._block(f"confirmation phrase mismatch; expected exactly: {expected}")
        self.request.confirm_phrase = phrase
        self.messages.append("write authorized by operator phrase")
        self.state = WriteState.WRITE_AUTHORIZED

    def execute(self) -> WriteAudit:
        self._require(WriteState.WRITE_AUTHORIZED)
        assert self.request is not None
        req = self.request
        op_id = str(uuid.uuid4())
        ts = datetime.now(timezone.utc).isoformat()

        if self.dry_run or req.dry_run:
            self.messages.append(
                "DRY-RUN: no vendor tool invoked; no firmware or table modified; "
                "not marked Verified"
            )
            self.state = WriteState.WRITE_AUTHORIZED
            return self._audit(op_id, ts, None)

        self.state = WriteState.EXECUTING

        if not req.vendor_tool_path or not Path(req.vendor_tool_path).is_file():
            self._block(
                "non-dry-run write requires --vendor-tool path to an operator-supplied "
                "utility; this tool does not embed AMIDEWIN or perform SPI DMI patching"
            )
        tool = Path(req.vendor_tool_path)

        if req.vendor_tool_sha256:
            actual = hashlib.sha256(tool.read_bytes()).hexdigest()
            if actual.lower() != req.vendor_tool_sha256.strip().lower():
                self._block(
                    f"vendor tool SHA-256 mismatch: expected {req.vendor_tool_sha256}, got {actual}"
                )
            self.messages.append(f"vendor tool hash verified: {actual}")
        else:
            self.messages.append(
                "WARNING: vendor tool SHA-256 not supplied; integrity not verified"
            )

        if not req.vendor_tool_args:
            self._block(
                "vendor_tool_args is empty; supply the exact switches for your utility"
            )

        self.messages.append(f"invoking vendor tool: {tool} {req.vendor_tool_args}")
        try:
            completed = subprocess.run(
                [str(tool), *req.vendor_tool_args],
                capture_output=True,
                text=True,
                timeout=120,
            )
        except OSError as e:
            self._block(f"failed to spawn vendor tool: {e}")

        code = completed.returncode
        self.messages.append(f"vendor tool exit={code}")
        if completed.stdout.strip():
            self.messages.append(f"stdout: {completed.stdout[:500]}")
        if completed.stderr.strip():
            self.messages.append(f"stderr: {completed.stderr[:500]}")
        if code != 0:
            self._block(f"vendor tool failed with exit {code}")

        self.messages.append(
            "vendor tool reported success — persistence is NOT proven until "
            "independent re-read after reboot"
        )
        self.state = WriteState.VERIFIED
        return self._audit(op_id, ts, code)

    def _audit(
        self, op_id: str, ts: str, exit_code: Optional[int]
    ) -> WriteAudit:
        req = self.request
        summary = ""
        vendor = None
        dry = self.dry_run
        if req:
            summary = f"board={req.exact_identity} fields={[u.field for u in req.updates]}"
            vendor = str(req.vendor_tool_path) if req.vendor_tool_path else None
            dry = self.dry_run or req.dry_run
        return WriteAudit(
            operation_id=op_id,
            state=self.state.value,
            request_summary=summary,
            backup_sha256=self.backup_sha256,
            vendor_tool=vendor,
            dry_run=dry,
            messages=list(self.messages),
            exit_code=exit_code,
            timestamp=ts,
        )


def run_write_workflow(req: WriteRequest) -> WriteAudit:
    coord = WriteCoordinator(dry_run=req.dry_run)
    coord.identify(req.board_model, req.board_rev, req.exact_identity)
    coord.verify_backup(req.backup_raw_path)
    coord.validate_input(req.updates)
    coord.confirm_recovery(req.recovery_plan)
    coord.authorize(req.confirm_phrase)
    assert coord.request is not None
    coord.request.vendor_tool_path = req.vendor_tool_path
    coord.request.vendor_tool_args = list(req.vendor_tool_args)
    coord.request.vendor_tool_sha256 = req.vendor_tool_sha256
    coord.request.dry_run = req.dry_run
    return coord.execute()
