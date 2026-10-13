"""
Platform capability report scaffold (Lesson 12).

States what is confirmed, unknown, and what recovery exists.
Does not implement persistent writes.
"""

from __future__ import annotations

from typing import Any, Dict


def platform_capability_template(
    board_model: str = "unknown",
    board_rev: str = "unknown",
) -> Dict[str, Any]:
    """
    Return a structured capability report that makes unknowns explicit.
    """
    return {
        "board_model": board_model,
        "board_rev": board_rev,
        "confirmed": {
            "published_smbios_read": (
                "Supported via GetSystemFirmwareTable(RSMB) on Windows "
                "and via saved raw captures offline."
            ),
            "safe_parsing": (
                "Bounded structure walk with string-set validation; "
                "malformed input produces errors instead of crashes."
            ),
        },
        "unknown_until_established": {
            "persistent_storage_of_type1_serial": (
                "Not inferred from the published table. May be firmware "
                "config, OEM region, manufacturing data, or non-writable."
            ),
            "supported_write_path": (
                "No universal SMBIOS write API. Possible paths (vendor DMI "
                "utility, OEM service, UEFI variable if actually used, "
                "factory workflow, SPI) must be confirmed per board."
            ),
            "amidewin_or_vendor_tool_compatibility": (
                "Version-, board-, and firmware-dependent. Must be verified "
                "with independent before/after reboot reads."
            ),
        },
        "recovery": {
            "required_before_any_write": [
                "Exact board model and PCB revision confirmed from physical label",
                "Verified same-board backup of relevant storage",
                "Documented recovery method (external programmer, dual BIOS, etc.)",
                "Explicit operator confirmation of identity and plan",
            ],
            "spi_note": (
                "Direct SPI programming is not a DMI update. Blind byte "
                "replacement in a flash image can corrupt firmware."
            ),
        },
        "policy": {
            "write_default": "disabled",
            "fail_closed": True,
            "layer_distinction": (
                "Published SMBIOS value ≠ persistent firmware source. "
                "Tools must not treat table appearance as proof of a write path."
            ),
        },
    }
