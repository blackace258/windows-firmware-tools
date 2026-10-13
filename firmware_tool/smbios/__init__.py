"""
SMBIOS / DMI table acquisition and parsing for Windows.

Reading a published SMBIOS table, changing a local copy, and changing the
firmware-managed source are distinct operations.

`write.py` is a fail-closed gate. It never patches SPI images. A non-dry-run
write only starts after every gate passes, and then only by launching an
operator-supplied vendor utility.
"""

from .acquisition import acquire_raw_table, load_raw_from_file, save_raw_table
from .parser import parse_smbios_table, ParseError
from .models import SmbiosReport, StructureRecord
from .compare import compare_reports
from .report import report_to_text, report_to_json

__all__ = [
    "acquire_raw_table",
    "load_raw_from_file",
    "save_raw_table",
    "parse_smbios_table",
    "ParseError",
    "SmbiosReport",
    "StructureRecord",
    "compare_reports",
    "report_to_text",
    "report_to_json",
]
