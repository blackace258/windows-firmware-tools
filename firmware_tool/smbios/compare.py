"""Compare two SMBIOS reports (published-table snapshots only)."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from .models import SmbiosReport


def _field_diff(
    label: str,
    a: Optional[Dict[str, Any]],
    b: Optional[Dict[str, Any]],
) -> List[str]:
    lines: List[str] = []
    if a is None and b is None:
        return lines
    if a is None:
        lines.append(f"  {label}: present only in B")
        return lines
    if b is None:
        lines.append(f"  {label}: present only in A")
        return lines
    keys = sorted(set(a.keys()) | set(b.keys()))
    for k in keys:
        if k.startswith("_"):
            continue
        va = a.get(k)
        vb = b.get(k)
        if va != vb:
            lines.append(f"  {label}.{k}: A={va!r}  B={vb!r}")
    return lines


def compare_reports(a: SmbiosReport, b: SmbiosReport) -> Dict[str, Any]:
    """
    Summarize differences between two published-table captures.

    Does not infer persistent firmware state.
    """
    diffs: List[str] = []
    same_hash = a.raw_sha256 == b.raw_sha256

    if a.raw_sha256 != b.raw_sha256:
        diffs.append(f"raw_sha256: A={a.raw_sha256}  B={b.raw_sha256}")
    if a.table_length != b.table_length:
        diffs.append(f"table_length: A={a.table_length}  B={b.table_length}")
    if (a.smbios_major, a.smbios_minor) != (b.smbios_major, b.smbios_minor):
        diffs.append(
            f"smbios_version: A={a.smbios_major}.{a.smbios_minor}  "
            f"B={b.smbios_major}.{b.smbios_minor}"
        )

    diffs.extend(_field_diff("type0_bios", a.type0, b.type0))
    diffs.extend(_field_diff("type1_system", a.type1, b.type1))
    diffs.extend(_field_diff("type2_baseboard", a.type2, b.type2))
    diffs.extend(_field_diff("type3_enclosure", a.type3, b.type3))

    return {
        "identical_raw": same_hash,
        "difference_count": len(diffs),
        "differences": diffs,
        "a_source": a.source,
        "b_source": b.source,
        "a_sha256": a.raw_sha256,
        "b_sha256": b.raw_sha256,
        "layer_note": (
            "Comparison is of published SMBIOS snapshots only. "
            "Identical published values do not prove identical firmware storage; "
            "differing values do not by themselves identify the write path used."
        ),
    }
