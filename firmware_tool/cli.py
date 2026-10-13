#!/usr/bin/env python3
"""
Motherboard BIOS/UEFI Firmware Inspection Tool — CLI entry point.

Firmware image commands are read-only. `smbios-write` is a separate fail-closed
gate that can launch an operator-supplied vendor utility; it does not patch SPI.
Windows-compatible (pathlib, no shell injection).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Optional

# Allow running as script or module
sys.path.insert(0, str(Path(__file__).resolve().parent))

from acquisition.file_source import FileSource
from acquisition.interface import FileAcquisitionError
from comparison.binary_diff import iter_diff_ranges
from models import Codes, FlashChip, InspectionReport, ValidationResult
from parsing.intel_descriptor import parse_intel_descriptor
from parsing.uefi import parse_uefi_image
from reporting.json_report import to_json, write_json
from reporting.text_report import to_text
from validation.compatibility import check_compatibility
from validation.hashes import compare_hashes, sha256_file
from validation.image_type import detect_image_type
from validation.size import check_size_against_capacity, suggest_common_capacity

from smbios.acquisition import (
    AcquisitionError,
    acquire_raw_table,
    load_raw_from_file,
    save_raw_table,
)
from smbios.parser import parse_smbios_table, ParseError
from smbios.report import report_to_text, report_to_json, compare_to_text
from smbios.compare import compare_reports
from smbios.capability import platform_capability_template
from smbios.write import (
    FieldUpdate,
    WriteBlocked,
    WriteRequest,
    run_write_workflow,
    CONFIRM_PHRASE_PREFIX,
)
import json


# Exit codes per specification
EXIT_OK = 0
EXIT_FAILED = 1
EXIT_INVALID = 2
EXIT_WARNINGS = 3
EXIT_HARDWARE = 4


def build_report(
    path: Path,
    expected_capacity: Optional[int] = None,
    parse_uefi: bool = True,
) -> InspectionReport:
    """Core inspection pipeline (read-only)."""
    source = FileSource(path)
    artifact = source.acquire()

    # Image type detection
    type_info = detect_image_type(path)
    artifact.artifact_type = type_info["artifact_type"]
    artifact.detected_format = type_info["detected_format"]

    report = InspectionReport(artifact=artifact)

    # Size / capacity
    size_result = check_size_against_capacity(
        artifact.size_bytes, expected_capacity
    )
    report.validation.size_check = size_result["status"]
    if size_result["status"] == "mismatch":
        report.add_finding(
            "warning",
            Codes.CAPACITY_MISMATCH,
            size_result["message"],
        )
    elif size_result["status"] == "unknown":
        suggested = suggest_common_capacity(artifact.size_bytes)
        if suggested:
            report.add_finding(
                "info",
                Codes.SIZE_REPORTED,
                f"Size matches a common SPI capacity ({suggested} bytes). "
                "Chip identity still required.",
            )
        else:
            report.add_finding(
                "info",
                Codes.SIZE_REPORTED,
                f"File size is {artifact.size_bytes} bytes. "
                "No chip capacity provided for comparison.",
            )

    # Chip unknown from file
    report.chip = FlashChip(identification_source="not identified")
    report.add_finding(
        "warning",
        Codes.CHIP_NOT_IDENTIFIED,
        "The image cannot be confirmed as a complete dump without chip information.",
    )

    # Compatibility
    compat = check_compatibility(artifact, report.chip)
    report.validation.compatibility = compat["status"]
    if compat["status"] == "not established":
        report.add_finding(
            "info",
            Codes.COMPATIBILITY_NOT_ESTABLISHED,
            "Compatibility not established: " + "; ".join(compat["reasons"]),
        )

    # Image type findings
    for ev in type_info.get("evidence", []):
        report.add_finding("info", Codes.UNKNOWN_IMAGE_TYPE if type_info["confidence"] in ("none", "low") else Codes.HASH_COMPUTED, ev)

    # Optional UEFI parse
    if parse_uefi:
        parse_uefi_image(path, report)
        parse_intel_descriptor(path, report)

    return report


def cmd_inspect(args: argparse.Namespace) -> int:
    path = Path(args.image)
    try:
        report = build_report(
            path,
            expected_capacity=args.capacity,
            parse_uefi=not args.no_parse,
        )
    except FileAcquisitionError as exc:
        print(f"ERROR [{exc.code}]: {exc.message}", file=sys.stderr)
        return EXIT_INVALID if exc.code in (Codes.FILE_NOT_FOUND, Codes.EMPTY_IMAGE) else EXIT_FAILED

    if args.format == "json":
        print(to_json(report))
    else:
        print(to_text(report))

    has_errors = any(f.severity == "error" for f in report.findings)
    has_warnings = any(f.severity == "warning" for f in report.findings)
    if has_errors:
        return EXIT_FAILED
    if has_warnings:
        return EXIT_WARNINGS
    return EXIT_OK


def cmd_hash(args: argparse.Namespace) -> int:
    path = Path(args.image)
    if not path.is_file():
        print(f"ERROR [{Codes.FILE_NOT_FOUND}]: {path}", file=sys.stderr)
        return EXIT_INVALID
    try:
        digest = sha256_file(path)
        size = path.stat().st_size
    except OSError as exc:
        print(f"ERROR [{Codes.READ_FAILED}]: {exc}", file=sys.stderr)
        return EXIT_FAILED
    print(f"path      : {path.resolve()}")
    print(f"size      : {size}")
    print(f"sha256    : {digest}")
    return EXIT_OK


def cmd_compare(args: argparse.Namespace) -> int:
    a = Path(args.image_a)
    b = Path(args.image_b)
    for p in (a, b):
        if not p.is_file():
            print(f"ERROR [{Codes.FILE_NOT_FOUND}]: {p}", file=sys.stderr)
            return EXIT_INVALID

    hash_cmp = compare_hashes(a, b)
    print("Hash comparison")
    print(f"  same_size     : {hash_cmp['same_size']}")
    print(f"  first_sha256  : {hash_cmp['first_sha256']}")
    print(f"  second_sha256 : {hash_cmp['second_sha256']}")
    print(f"  identical     : {hash_cmp['identical']}")

    if args.byte_ranges:
        diff = iter_diff_ranges(a, b, max_ranges=args.max_ranges)
        print("\nByte-range comparison")
        print(f"  differing_bytes : {diff.get('differing_bytes')}")
        print(f"  identical       : {diff.get('identical')}")
        print(f"  truncated       : {diff.get('truncated')}")
        for r in diff.get("ranges", []):
            print(f"  range [{r['start']:#x} .. {r['end']:#x}] len={r['length']}")

    if hash_cmp["identical"]:
        return EXIT_OK
    return EXIT_WARNINGS


def cmd_parse_uefi(args: argparse.Namespace) -> int:
    path = Path(args.image)
    try:
        report = build_report(path, parse_uefi=True)
    except FileAcquisitionError as exc:
        print(f"ERROR [{exc.code}]: {exc.message}", file=sys.stderr)
        return EXIT_INVALID

    if args.output:
        write_json(report, Path(args.output))
        print(f"Report written to {args.output}")
    else:
        print(to_text(report))

    has_errors = any(f.severity == "error" for f in report.findings)
    return EXIT_FAILED if has_errors else EXIT_OK


def cmd_identify(args: argparse.Namespace) -> int:
    path = Path(args.image)
    if not path.is_file():
        print(f"ERROR [{Codes.FILE_NOT_FOUND}]: {path}", file=sys.stderr)
        return EXIT_INVALID
    info = detect_image_type(path)
    print(f"artifact_type   : {info['artifact_type']}")
    print(f"detected_format : {info['detected_format']}")
    print(f"confidence      : {info['confidence']}")
    print("evidence:")
    for e in info["evidence"]:
        print(f"  - {e}")
    return EXIT_OK


def cmd_report(args: argparse.Namespace) -> int:
    # Alias for inspect with explicit format
    args.format = args.format or "text"
    args.no_parse = False
    args.capacity = None
    return cmd_inspect(args)


def cmd_smbios_read(args: argparse.Namespace) -> int:
    """Read published SMBIOS table (Windows RSMB or saved file)."""
    try:
        if args.file:
            raw, source = load_raw_from_file(Path(args.file))
        else:
            raw, source = acquire_raw_table()
    except AcquisitionError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return EXIT_FAILED

    if args.save_raw:
        save_raw_table(raw, Path(args.save_raw))
        print(f"Raw table saved: {args.save_raw} ({len(raw)} bytes)")

    try:
        report = parse_smbios_table(raw, source=source)
    except ParseError as e:
        print(f"ERROR: parse failed: {e}", file=sys.stderr)
        return EXIT_INVALID

    if args.format == "json":
        text = report_to_json(report)
        if args.output:
            Path(args.output).write_text(text, encoding="utf-8")
            print(f"JSON report written to {args.output}")
        else:
            print(text)
    else:
        print(report_to_text(report))
        if args.output:
            Path(args.output).write_text(report_to_json(report), encoding="utf-8")
            print(f"\nJSON also written to {args.output}")

    if report.parse_errors:
        return EXIT_WARNINGS
    return EXIT_OK


def cmd_smbios_compare(args: argparse.Namespace) -> int:
    """Compare two published SMBIOS captures."""
    try:
        raw_a, src_a = load_raw_from_file(Path(args.a))
        raw_b, src_b = load_raw_from_file(Path(args.b))
        rep_a = parse_smbios_table(raw_a, source=src_a)
        rep_b = parse_smbios_table(raw_b, source=src_b)
    except (AcquisitionError, ParseError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return EXIT_INVALID

    cmp = compare_reports(rep_a, rep_b)
    if args.format == "json":
        print(json.dumps(cmp, indent=2))
    else:
        print(compare_to_text(cmp))
    return EXIT_OK if cmp.get("identical_raw") else EXIT_WARNINGS


def cmd_smbios_capability(args: argparse.Namespace) -> int:
    """Emit platform capability report (gated write; unknowns explicit)."""
    report = platform_capability_template(
        board_model=args.board_model,
        board_rev=args.board_rev,
    )
    text = json.dumps(report, indent=2)
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
        print(f"Capability report written to {args.output}")
    else:
        print(text)
    return EXIT_OK


def cmd_smbios_write(args: argparse.Namespace) -> int:
    """Gated DMI write via operator-supplied vendor tool (fail-closed)."""
    if len(args.field) != len(args.value):
        print("ERROR: --field and --value counts must match", file=sys.stderr)
        return EXIT_INVALID
    identity = args.exact_identity or f"{args.board_model} PCB:{args.board_rev}"
    updates = [
        FieldUpdate(field=f, new_value=v)
        for f, v in zip(args.field, args.value)
    ]
    req = WriteRequest(
        board_model=args.board_model,
        board_rev=args.board_rev,
        exact_identity=identity,
        updates=updates,
        backup_raw_path=Path(args.backup),
        recovery_plan=args.recovery_plan,
        confirm_phrase=args.confirm_write,
        dry_run=args.dry_run,
        vendor_tool_path=Path(args.vendor_tool) if args.vendor_tool else None,
        vendor_tool_args=list(args.vendor_arg or []),
        vendor_tool_sha256=args.vendor_sha256,
    )
    print("=== gated DMI write workflow ===")
    print(f"exact identity: {identity}")
    print(f"required phrase: {CONFIRM_PHRASE_PREFIX} {identity}")
    print(f"dry_run: {args.dry_run}")
    try:
        audit = run_write_workflow(req)
    except WriteBlocked as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return EXIT_FAILED
    print(f"operation_id: {audit.operation_id}")
    print(f"final state: {audit.state}")
    for m in audit.messages:
        print(f"  • {m}")
    if args.audit_out:
        Path(args.audit_out).write_text(
            json.dumps(audit.to_dict(), indent=2), encoding="utf-8"
        )
        print(f"audit saved: {args.audit_out}")
    print(
        "\nPersistence is only proven by an independent re-read after reboot:\n"
        "  python cli.py smbios-read --save-raw after.bin\n"
        "  python cli.py smbios-compare <backup> after.bin"
    )
    return EXIT_OK


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        prog="firmware-tool",
        description=(
            "Motherboard BIOS/UEFI Firmware Inspection Tool. "
            "Image ops are read-only. SMBIOS write is gated and fail-closed."
        ),
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # inspect
    p_insp = sub.add_parser("inspect", help="Inspect a firmware image (read-only)")
    p_insp.add_argument("image", help="Path to firmware binary")
    p_insp.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
        help="Output format",
    )
    p_insp.add_argument(
        "--capacity",
        type=int,
        default=None,
        help="Expected chip capacity in bytes (optional)",
    )
    p_insp.add_argument(
        "--no-parse",
        action="store_true",
        help="Skip UEFI structure parsing",
    )
    p_insp.set_defaults(func=cmd_inspect)

    # hash
    p_hash = sub.add_parser("hash", help="Compute SHA-256 of an image")
    p_hash.add_argument("image", help="Path to firmware binary")
    p_hash.set_defaults(func=cmd_hash)

    # compare
    p_cmp = sub.add_parser("compare", help="Compare two images (read-only)")
    p_cmp.add_argument("image_a", help="First image")
    p_cmp.add_argument("image_b", help="Second image")
    p_cmp.add_argument(
        "--byte-ranges",
        action="store_true",
        help="Also report differing byte ranges",
    )
    p_cmp.add_argument(
        "--max-ranges",
        type=int,
        default=50,
        help="Max number of diff ranges to display",
    )
    p_cmp.set_defaults(func=cmd_compare)

    # parse-uefi
    p_uefi = sub.add_parser("parse-uefi", help="Parse UEFI structures")
    p_uefi.add_argument("image", help="Path to firmware binary")
    p_uefi.add_argument(
        "--output",
        help="Write JSON report to this path",
    )
    p_uefi.set_defaults(func=cmd_parse_uefi)

    # identify-package
    p_id = sub.add_parser("identify-package", help="Identify image type")
    p_id.add_argument("image", help="Path to firmware binary")
    p_id.set_defaults(func=cmd_identify)

    # report
    p_rep = sub.add_parser("report", help="Full report (alias of inspect)")
    p_rep.add_argument("image", help="Path to firmware binary")
    p_rep.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
    )
    p_rep.set_defaults(func=cmd_report)

    # --- SMBIOS / DMI (read-only published table) ---
    p_smbios = sub.add_parser(
        "smbios-read",
        help="Read published SMBIOS table (Windows GetSystemFirmwareTable or file)",
    )
    p_smbios.add_argument(
        "--file",
        help="Load raw capture from file instead of live Windows RSMB",
    )
    p_smbios.add_argument(
        "--save-raw",
        help="Save raw table bytes to this path",
    )
    p_smbios.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
    )
    p_smbios.add_argument(
        "--output",
        help="Write JSON report to this path",
    )
    p_smbios.set_defaults(func=cmd_smbios_read)

    p_smbios_cmp = sub.add_parser(
        "smbios-compare",
        help="Compare two saved SMBIOS raw captures",
    )
    p_smbios_cmp.add_argument("a", help="First raw capture")
    p_smbios_cmp.add_argument("b", help="Second raw capture")
    p_smbios_cmp.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
    )
    p_smbios_cmp.set_defaults(func=cmd_smbios_compare)

    p_cap = sub.add_parser(
        "smbios-capability",
        help="Platform capability report (gated write; unknowns explicit)",
    )
    p_cap.add_argument("--board-model", default="unknown")
    p_cap.add_argument("--board-rev", default="unknown")
    p_cap.add_argument("--output", help="Write JSON to path")
    p_cap.set_defaults(func=cmd_smbios_capability)

    p_wr = sub.add_parser(
        "smbios-write",
        help="Gated DMI write via operator-supplied vendor tool (fail-closed)",
    )
    p_wr.add_argument("--board-model", required=True)
    p_wr.add_argument("--board-rev", required=True)
    p_wr.add_argument("--exact-identity", default=None)
    p_wr.add_argument(
        "--field",
        action="append",
        required=True,
        help="Field key e.g. type1.serial_number (repeatable)",
    )
    p_wr.add_argument(
        "--value",
        action="append",
        required=True,
        help="New value for each --field (same count)",
    )
    p_wr.add_argument("--backup", required=True, help="Path to raw SMBIOS backup")
    p_wr.add_argument("--recovery-plan", required=True)
    p_wr.add_argument(
        "--confirm-write",
        required=True,
        help='Exact phrase: I AUTHORIZE DMI WRITE ON <exact identity>',
    )
    p_wr.add_argument("--vendor-tool", default=None)
    p_wr.add_argument("--vendor-arg", action="append", default=[])
    p_wr.add_argument("--vendor-sha256", default=None)
    p_wr.add_argument("--dry-run", action="store_true", default=False)
    p_wr.add_argument("--audit-out", default=None)
    p_wr.set_defaults(func=cmd_smbios_write)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
