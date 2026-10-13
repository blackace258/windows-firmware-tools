//! Platform capability report. States what is confirmed vs unknown.
//! Writes live in `crate::write`, not here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilityReport {
    pub board_family: String,
    pub pcb_revision_reported: String,
    pub flash_marking_reported: String,
    pub confirmed: Vec<String>,
    pub unknown: Vec<String>,
    pub vendor_documentation: Vec<String>,
    pub recovery_method: Vec<String>,
    pub write_support: String,
    pub recommendation: String,
}

pub fn gigabyte_b760m_aorus_rev11() -> PlatformCapabilityReport {
    PlatformCapabilityReport {
        board_family: "Gigabyte B760M AORUS family (exact retail SKU provisional)".into(),
        pcb_revision_reported: "1.1".into(),
        flash_marking_reported: "Macronix MX25L25673G family (256 Mbit / 32 MiB, 3-V-class) — confirm full marking".into(),
        confirmed: vec![
            "Published SMBIOS table is readable on Windows via GetSystemFirmwareTable(RSMB).".into(),
            "This tool can acquire, hash, parse (Types 0–3), compare, and emit JSON for the published table.".into(),
            "SPI full-chip dumps are handled by the separate bios-tool project (flashrom backend).".into(),
        ],
        unknown: vec![
            "Exact retail SKU / product label text on the physical PCB.".into(),
            "Complete flash part number suffix and whether it matches the provisional MX25L25673G identification.".into(),
            "Whether any Gigabyte or AMI DMI utility is supported and authorized for this exact board/firmware revision.".into(),
            "Whether any Type 1/2 serial or UUID fields are stored in UEFI variables, a dedicated OEM region, or only constructed at boot.".into(),
            "Whether a manufacturing-mode or vendor service interface exists for persistent DMI updates.".into(),
            "Write-protection state of the SPI flash and descriptor permissions.".into(),
        ],
        vendor_documentation: vec![
            "Consult Gigabyte support / download page for the exact model BIOS utilities and DMI tools, if any.".into(),
            "AMI tools (e.g. AMIDEWIN) are version- and platform-dependent; do not assume support without verification on this board.".into(),
            "UEFI specification defines variable services, but SMBIOS fields are not automatically UEFI variables.".into(),
        ],
        recovery_method: vec![
            "Hardware SPI programmer + verified full-chip backup (see bios-tool dump workflow).".into(),
            "Motherboard dual-BIOS or recovery procedure if documented for the exact SKU.".into(),
            "Never attempt a write without a confirmed recovery path and same-board backup.".into(),
        ],
        write_support: "GATED. smbios-tool write uses a mandatory safety state machine and only invokes an operator-supplied vendor utility after backup, recovery plan, exact confirmation phrase, and optional tool hash checks. No SPI byte-patching for DMI. Fail-closed by default; dry-run available.".into(),
        recommendation: "1) Confirm exact SKU and flash marking from the physical board. 2) Capture and archive raw SMBIOS (this tool) and full SPI image (bios-tool). 3) Identify any vendor-supported DMI utility and verify it against independent before/after reboot reads. 4) Do not perform SPI byte-patching for DMI fields unless layout, protection, and recovery are fully understood.".into(),
    }
}

/// Profile string did not match a built-in board. Do not pretend it is the B760M report.
pub fn unknown_profile(profile: &str) -> PlatformCapabilityReport {
    PlatformCapabilityReport {
        board_family: format!("unrecognized profile '{profile}'"),
        pcb_revision_reported: "unknown".into(),
        flash_marking_reported: "unknown — do not reuse another board's flash identity".into(),
        confirmed: vec![],
        unknown: vec![
            "No built-in capability data exists for this profile.".into(),
            "Board model, PCB revision, flash part, and DMI write mechanism are all unverified.".into(),
        ],
        vendor_documentation: vec![
            "Look up the exact retail SKU. Do not apply the Gigabyte B760M AORUS rev 1.1 notes to a different board.".into(),
        ],
        recovery_method: vec![
            "Not evaluated. A hardware programmer and a same-board backup are required before any write on an unknown board.".into(),
        ],
        write_support: "NOT EVALUATED. Refusing to report the B760M write gate as if it applied to this profile.".into(),
        recommendation: "Re-run with --profile gigabyte-b760m-aorus-rev1.1 only when that is the physical board, or extend capability.rs after the label is confirmed.".into(),
    }
}

pub fn print_capability(r: &PlatformCapabilityReport) {
    println!("=== Platform Capability Report ===");
    println!("Board family:     {}", r.board_family);
    println!("PCB revision:     {}", r.pcb_revision_reported);
    println!("Flash marking:    {}", r.flash_marking_reported);
    println!();
    println!("CONFIRMED:");
    for c in &r.confirmed {
        println!("  • {}", c);
    }
    println!();
    println!("UNKNOWN / UNVERIFIED:");
    for u in &r.unknown {
        println!("  • {}", u);
    }
    println!();
    println!("VENDOR / SPEC NOTES:");
    for v in &r.vendor_documentation {
        println!("  • {}", v);
    }
    println!();
    println!("RECOVERY:");
    for v in &r.recovery_method {
        println!("  • {}", v);
    }
    println!();
    println!("WRITE SUPPORT: {}", r.write_support);
    println!();
    println!("RECOMMENDATION: {}", r.recommendation);
}
