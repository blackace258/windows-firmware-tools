"""
Acquire the published SMBIOS table.

On Windows: GetSystemFirmwareTable with provider 'RSMB'.
Offline: load a previously saved raw capture (for test/reproducibility).

Never writes. Treats firmware bytes as untrusted input.
"""

from __future__ import annotations

import ctypes
import platform
import struct
import sys
from pathlib import Path
from typing import Optional, Tuple

# Windows GetSystemFirmwareTable provider signature for raw SMBIOS
# FirmwareTableProviderSignature 'RSMB' = 0x52534D42
RSMB_PROVIDER = 0x52534D42


class AcquisitionError(Exception):
    """Failed to obtain the SMBIOS table."""


def _is_windows() -> bool:
    return sys.platform == "win32" or platform.system().lower() == "windows"


def acquire_raw_table() -> Tuple[bytes, str]:
    """
    Read the published raw SMBIOS table from the OS.

    Returns:
        (raw_bytes, source_description)

    Raises:
        AcquisitionError if the platform is unsupported or the API fails.
    """
    if not _is_windows():
        raise AcquisitionError(
            "GetSystemFirmwareTable(RSMB) is only available on Windows. "
            "On non-Windows hosts, use load_raw_from_file() with a saved capture."
        )
    return _acquire_windows_rsmb()


def _acquire_windows_rsmb() -> Tuple[bytes, str]:
    """Call kernel32.GetSystemFirmwareTable for RSMB."""
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

    GetSystemFirmwareTable = kernel32.GetSystemFirmwareTable
    GetSystemFirmwareTable.argtypes = [
        ctypes.c_uint32,  # FirmwareTableProviderSignature
        ctypes.c_uint32,  # FirmwareTableID
        ctypes.c_void_p,  # pFirmwareTableBuffer
        ctypes.c_uint32,  # BufferSize
    ]
    GetSystemFirmwareTable.restype = ctypes.c_uint32

    # First call: size query
    size_needed = GetSystemFirmwareTable(RSMB_PROVIDER, 0, None, 0)
    if size_needed == 0:
        err = ctypes.get_last_error()
        raise AcquisitionError(
            f"GetSystemFirmwareTable size query failed (GetLastError={err})"
        )

    buf = (ctypes.c_ubyte * size_needed)()
    written = GetSystemFirmwareTable(
        RSMB_PROVIDER, 0, ctypes.byref(buf), size_needed
    )
    if written == 0:
        err = ctypes.get_last_error()
        raise AcquisitionError(
            f"GetSystemFirmwareTable read failed (GetLastError={err})"
        )
    if written > size_needed:
        raise AcquisitionError(
            f"GetSystemFirmwareTable returned size {written} > allocated {size_needed}"
        )

    data = bytes(buf[:written])
    source = "GetSystemFirmwareTable(RSMB)"
    return data, source


def load_raw_from_file(path: Path) -> Tuple[bytes, str]:
    """Load a previously saved raw SMBIOS capture from disk."""
    path = Path(path)
    if not path.is_file():
        raise AcquisitionError(f"File not found: {path}")
    data = path.read_bytes()
    if len(data) < 8:
        raise AcquisitionError(
            f"File too small for SMBIOS header ({len(data)} bytes): {path}"
        )
    return data, f"file:{path.resolve()}"


def save_raw_table(data: bytes, path: Path) -> None:
    """Preserve raw bytes for reproducibility. Does not modify firmware."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def parse_rsmb_header(data: bytes) -> Tuple[int, int, int, int, bytes]:
    """
    Parse the Windows RSMB buffer header.

    Layout (Windows GetSystemFirmwareTable RSMB):
      DWORD UsedCallingMethod (or similar) / often documented as:
      Actually standard layout from MSDN / SMBIOS:
        BYTE  Used20CallingMethod
        BYTE  MajorVersion
        BYTE  MinorVersion
        BYTE  DmiRevision
        DWORD Length
        BYTE  SMBIOSTableData[Length]

    Some docs show:
      Offset 0: UsedCallingMethod (1 byte)
      Offset 1: SMBIOSMajorVersion
      Offset 2: SMBIOSMinorVersion
      Offset 3: DmiRevision
      Offset 4: Length (4 bytes LE)
      Offset 8: table data

    Returns: (major, minor, dmi_rev, table_length, table_body)
    """
    if len(data) < 8:
        raise AcquisitionError(
            f"RSMB buffer too short: {len(data)} bytes (need at least 8)"
        )

    used = data[0]
    major = data[1]
    minor = data[2]
    dmi_rev = data[3]
    table_length = struct.unpack_from("<I", data, 4)[0]

    if table_length > len(data) - 8:
        # Bound: do not trust firmware length blindly
        raise AcquisitionError(
            f"RSMB claimed table length {table_length} exceeds "
            f"available data ({len(data) - 8} bytes after header)"
        )

    body = data[8 : 8 + table_length]
    return major, minor, dmi_rev, table_length, body
