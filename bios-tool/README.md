# bios-tool

Windows-native **CLI + professional GUI** for SPI BIOS extraction, firmware inspection, verification, and **controlled** programming / erase.

**Primary target (provisional):** Gigabyte B760M AORUS family, PCB revision 1.1.
**Reported flash marking:** Macronix MX25L25673G family (256 Mbit / 32 MiB, 3-V-class).

Exact retail SKU and complete flash part number **must** be confirmed from the physical PCB/product label before any write. The application never silently normalizes similar model names.

## Safety model

Writes and erases are gated by an explicit state machine:

```
Unidentified → Identified → BackupVerified → ImageValidated → RecoveryConfirmed
→ WriteAuthorized → Writing → ReadbackVerified
```

Any missing or failed condition transitions to **Blocked**. There is no generic `--yes` bypass. The confirmation phrase must be the exact board identity string. Force/unlock/protection-bypass flags are never added automatically.

Hard blockers include uncertain board/PCB revision, uncertain flash capacity, missing or unverified same-board backup, capsule-vs-raw confusion, unsupported backend, protection errors, and missing recovery plan.

## Requirements

- Windows 10/11 x64
- Rust stable (MSVC toolchain) for building
- A **Windows-compatible** flashrom build and programmer that is actually supported on Windows for your hardware. This project uses flashrom only as a child process; it does not implement a custom SPI driver.
- Programmer drivers from trusted vendor sources only.

### Bootstrap (check / install / build)

```powershell
.\setup_windows.ps1
.\setup_windows.ps1 -CheckOnly
.\setup_windows.ps1 -Install
.\setup_windows.ps1 -Build
.\setup_windows.ps1 -Install -Build
```

After a successful `-Build`, the CLI binary is at:

```text
target\release\bios-tool.exe
```

### Build with professional GUI

```powershell
cargo build --release --features gui --bin bios-tool-gui
# → target\release\bios-tool-gui.exe
```

## CLI overview

| Command | Purpose |
|---------|---------|
| `bios-tool doctor` | Environment and dependency report (no writes) |
| `bios-tool setup-check` | Same detection logic; never installs or flashes |
| `bios-tool device probe` | Read-only chip/programmer identification |
| `bios-tool dump --output <path> --reads 2` | Multi-read full-chip dump; saves only if identical |
| `bios-tool inspect <image>` | Read-only size/hash/signature/structure report |
| `bios-tool compare <a> <b>` | Hash and identity comparison |
| `bios-tool validate --image <path> --profile <id>` | Board/profile/layout checks |
| `bios-tool verify --image <path>` | Chip vs image (no write) |
| `bios-tool write ...` | Safety-gated program + immediate read-back |
| `bios-tool erase ...` | Safety-gated full-chip erase |
| `bios-tool report <operation-id>` | Audit JSON |

All meaningful operations accept `--dry-run` where applicable.

## GUI features

- Dark professional theme
- Sidebar navigation: Dashboard · Probe · Dump · Inspect · Validate · Write · Erase · Logs
- Native file pickers
- Live safety-state visualization
- Exact confirmation-phrase entry
- Dry-run toggle always visible
- Automatic audit JSON under `logs\`

## License

MIT
