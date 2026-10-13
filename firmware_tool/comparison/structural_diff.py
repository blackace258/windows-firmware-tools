"""Structural comparison of parsed firmware models (placeholder for Phase 3+)."""

from __future__ import annotations

from models import InspectionReport


def structural_compare(
    report_a: InspectionReport,
    report_b: InspectionReport,
) -> dict:
    """
    High-level structural comparison.

    Full GUID/region matching will be expanded once UEFI parsing is mature.
    """
    regions_a = {r.name: r for r in report_a.regions}
    regions_b = {r.name: r for r in report_b.regions}

    only_a = sorted(set(regions_a) - set(regions_b))
    only_b = sorted(set(regions_b) - set(regions_a))
    common = sorted(set(regions_a) & set(regions_b))

    changed = []
    for name in common:
        ra = regions_a[name]
        rb = regions_b[name]
        if ra.offset != rb.offset or ra.size != rb.size:
            changed.append(
                {
                    "name": name,
                    "a": {"offset": ra.offset, "size": ra.size},
                    "b": {"offset": rb.offset, "size": rb.size},
                }
            )

    return {
        "regions_only_in_a": only_a,
        "regions_only_in_b": only_b,
        "regions_changed": changed,
        "regions_common": common,
        "note": (
            "Structural comparison is limited until full UEFI parsing is available. "
            "Byte differences may exist even when logical structures appear similar."
        ),
    }
