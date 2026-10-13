"""Machine-readable JSON report generation."""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path
from typing import Any

from models import InspectionReport


def _serialize(obj: Any) -> Any:
    """Convert dataclasses and Paths to JSON-serializable forms."""
    if hasattr(obj, "__dataclass_fields__"):
        return {k: _serialize(v) for k, v in asdict(obj).items()}
    if isinstance(obj, Path):
        return str(obj)
    if isinstance(obj, list):
        return [_serialize(i) for i in obj]
    if isinstance(obj, dict):
        return {k: _serialize(v) for k, v in obj.items()}
    return obj


def to_json(report: InspectionReport, indent: int = 2) -> str:
    """Serialize InspectionReport to a JSON string."""
    data = _serialize(report)
    return json.dumps(data, indent=indent, ensure_ascii=False)


def write_json(report: InspectionReport, output_path: Path) -> None:
    """Write report to a JSON file."""
    output_path = Path(output_path)
    text = to_json(report)
    output_path.write_text(text, encoding="utf-8")
