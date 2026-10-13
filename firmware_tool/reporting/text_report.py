"""Human-readable text report generation."""

from __future__ import annotations

from models import InspectionReport


def to_text(report: InspectionReport) -> str:
    """Produce a human-readable inspection report."""
    lines: list[str] = []
    art = report.artifact

    lines.append("=" * 60)
    lines.append("FIRMWARE INSPECTION REPORT")
    lines.append("=" * 60)
    lines.append("")
    lines.append("Artifact")
    lines.append(f"  Path          : {art.path}")
    lines.append(f"  Size (bytes)  : {art.size_bytes}")
    lines.append(f"  SHA-256       : {art.sha256}")
    lines.append(f"  Artifact type : {art.artifact_type}")
    if art.detected_format:
        lines.append(f"  Detected fmt  : {art.detected_format}")
    if art.timestamp_utc:
        lines.append(f"  Inspected at  : {art.timestamp_utc}")
    lines.append("")

    lines.append("Chip")
    if report.chip:
        c = report.chip
        lines.append(f"  Manufacturer  : {c.manufacturer or 'n/a'}")
        lines.append(f"  Part number   : {c.part_number or 'n/a'}")
        lines.append(f"  Capacity      : {c.capacity_bytes or 'n/a'} bytes")
        lines.append(f"  Voltage       : {c.voltage_requirements or 'n/a'}")
        lines.append(f"  Ident source  : {c.identification_source}")
    else:
        lines.append("  (not identified)")
    lines.append("")

    lines.append("Validation")
    v = report.validation
    lines.append(f"  Size check        : {v.size_check}")
    lines.append(f"  Repeat-read check : {v.repeat_read_check}")
    lines.append(f"  Compatibility     : {v.compatibility}")
    lines.append("")

    if report.regions:
        lines.append(f"Regions ({len(report.regions)})")
        for r in report.regions:
            guid = f"  GUID={r.guid}" if r.guid else ""
            lines.append(
                f"  [{r.offset:#010x} + {r.size:#x}] {r.name}"
                f"  fmt={r.format_name or 'n/a'}{guid}"
            )
        lines.append("")
    else:
        lines.append("Regions: (none parsed)")
        lines.append("")

    if report.findings:
        lines.append(f"Findings ({len(report.findings)})")
        for f in report.findings:
            off = f" @ {f.offset:#x}" if f.offset is not None else ""
            lines.append(f"  [{f.severity.upper()}] {f.code}{off}")
            lines.append(f"    {f.message}")
        lines.append("")
    else:
        lines.append("Findings: (none)")
        lines.append("")

    lines.append("-" * 60)
    lines.append(
        "Note: Byte-for-byte equality does not prove firmware compatibility "
        "or successful boot. Chip identity and board documentation are required."
    )
    lines.append("=" * 60)
    return "\n".join(lines)
