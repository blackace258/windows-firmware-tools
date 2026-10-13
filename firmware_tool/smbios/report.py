"""Text and JSON reporting for SMBIOS captures."""

from __future__ import annotations

import json
from typing import Any, Dict

from .models import SmbiosReport
from .compare import compare_reports


def report_to_text(report: SmbiosReport) -> str:
    lines = [
        "=== SMBIOS / DMI published table report ===",
        f"source:          {report.source}",
        f"platform:        {report.platform}",
        f"SMBIOS version:  {report.smbios_major}.{report.smbios_minor}",
        f"DMI revision:    {report.dmi_revision}",
        f"table length:    {report.table_length}",
        f"raw size:        {report.raw_size}",
        f"raw SHA-256:     {report.raw_sha256}",
        f"structures:      {len(report.structures)}",
        "",
        report.layer_note,
        "",
    ]

    def _block(title: str, data: dict | None) -> None:
        lines.append(f"--- {title} ---")
        if not data:
            lines.append("  (not present or not parsed)")
            lines.append("")
            return
        for k, v in data.items():
            if k.startswith("_"):
                continue
            lines.append(f"  {k}: {v}")
        lines.append("")

    _block("Type 0 — BIOS Information", report.type0)
    _block("Type 1 — System Information", report.type1)
    _block("Type 2 — Baseboard Information", report.type2)
    _block("Type 3 — System Enclosure", report.type3)

    if report.parse_warnings:
        lines.append("--- parse warnings ---")
        for w in report.parse_warnings:
            lines.append(f"  WARN: {w}")
        lines.append("")
    if report.parse_errors:
        lines.append("--- parse errors ---")
        for e in report.parse_errors:
            lines.append(f"  ERROR: {e}")
        lines.append("")

    lines.append(
        "NOTE: Serial numbers and UUID above are published table values only. "
        "Changing a local copy of this report does not alter firmware storage."
    )
    return "\n".join(lines)


def report_to_json(report: SmbiosReport, indent: int = 2) -> str:
    return json.dumps(report.to_dict(), indent=indent, ensure_ascii=False)


def compare_to_text(cmp: Dict[str, Any]) -> str:
    lines = [
        "=== SMBIOS capture comparison ===",
        f"A source: {cmp.get('a_source')}",
        f"B source: {cmp.get('b_source')}",
        f"identical raw: {cmp.get('identical_raw')}",
        f"differences: {cmp.get('difference_count')}",
        "",
    ]
    for d in cmp.get("differences") or []:
        lines.append(f"  * {d}")
    if not cmp.get("differences"):
        lines.append("  (no field differences reported)")
    lines.append("")
    lines.append(cmp.get("layer_note", ""))
    return "\n".join(lines)
