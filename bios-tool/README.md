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
# 1. Detect required tools (Rust, MSVC linker, optional flashrom, etc.)
.\setup_windows.ps1
# or:  .\setup_windows.ps1 -CheckOnly

# 2. Guided install of missing tools (consent required for every step)
.\setup_windows.ps1 -Install

# 3. Build the release binary once dependencies are present
.\setup_windows.ps1 -Build

# Combined: guidance then build
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

The GUI uses egui/eframe (immediate-mode, dark professional theme, native file dialogs). It exposes every safety-gated path (Probe, Dump, Inspect, Validate, Write, Erase) with live state-machine visualization and exact-phrase confirmation.

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
| `bios-tool write --image <path> --backup <same-board-backup> --confirm-write "<exact board identity>" --board-model "…" --chip-part "…"` | Safety-gated program + immediate read-back |
| `bios-tool erase --backup <path> --confirm-erase "<exact board identity>" --board-model "…" --chip-part "…"` | Safety-gated full-chip erase |
| `bios-tool report <operation-id>` | Audit JSON |

All meaningful operations accept `--dry-run` where applicable. Dry-run never invokes a write backend operation.

### Write example (do not run without verified hardware readiness)

```text
bios-tool write ^
  --image candidate.bin ^
  --backup board-backup.bin ^
  --confirm-write "Gigabyte B760M AORUS PCB:1.1" ^
  --board-model "B760M AORUS" ^
  --board-rev "1.1" ^
  --chip-part "MX25L25673G" ^
  --chip-size 33554432 ^
  --recovery-plan "external programmer + known-good dump"
```

### Erase example

```text
bios-tool erase ^
  --backup board-backup.bin ^
  --confirm-erase "Gigabyte B760M AORUS PCB:1.1" ^
  --board-model "B760M AORUS" ^
  --chip-part "MX25L25673G" ^
  --recovery-plan "external programmer + known-good dump"
```

## GUI features

- Dark professional theme optimized for long sessions
- Sidebar navigation: Dashboard · Probe · Dump · Inspect · Validate · Write · Erase · Logs
- Native file pickers (rfd)
- Live safety-state visualization
- Exact confirmation-phrase entry for every destructive action
- Dry-run toggle always visible
- Automatic audit JSON under `logs\`

## Programmer support note

Windows support for flashrom programmers varies by release and driver. The `internal` programmer is often limited or unavailable on Windows. External programmers (e.g. CH341A, FT2232) require verified Windows binaries and signed drivers. The tool reports unsupported states; it does not silently fall back to a home-grown driver.

## Offline / no-hardware use

`inspect`, `compare`, `validate`, and pure parsing paths work without flashrom or elevated privileges. `doctor` and `setup-check` report what is missing without installing. The GUI can be used fully offline for inspection and validation.

## Limits of the current release

- Firmware structure parsing is **read-only**. No NVRAM, MAC, serial, UUID, ME region, descriptor permission, microcode, or Secure Boot key editing.
- Board profiles are provisional; exact SKU must be operator-confirmed.
- CI and unit tests never touch physical hardware; backend is mockable.
- Actual hardware success is never claimed without evidence from a real probe/write on the target board.

## Project layout

```
src/
  cli/           command parsing (write + erase with explicit identity flags)
  domain/        board, chip, image, safety state machine, errors
  backend/       flashrom process integration (read / write / erase / verify)
  setup/         dependency diagnostics
  acquisition/   multi-read dump + metadata
  firmware/      bounded descriptor / UEFI FV / signature scanners
  validation/    profile and layout checks
  programming/   gated write & erase coordinator
  audit/         operation records
  gui/           professional egui UI (feature = "gui")
  bin/gui.rs     GUI binary entry point
```

## License

MIT

## SPI read / write (flashrom backend)

All SPI access is through **flashrom as a child process**. No custom kernel SPI driver is shipped.

### Environment (Windows)

```powershell
$env:FLASHROM_PATH = "C:\path\to\flashrom.exe"
$env:FLASHROM_PROGRAMMER = "ch341a_spi"   # or ft2232_spi, etc. — must match your hardware
```

Use only a flashrom build and programmer drivers you have verified. `internal` is often limited or unavailable on Windows.

### Read (full-chip dump)

```powershell
# Probe programmer / chip (read-only)
.\target\release\bios-tool.exe device probe

# Two consistent full-chip reads; refuses to save if they differ
.\target\release\bios-tool.exe dump --output board_backup.bin --reads 2

# Or use the helper script
.\scripts\spi_workflow.ps1 -Read -Output board_backup.bin
```

### Write (gated)

Write is blocked unless:

1. Board and chip identities are operator-confirmed (exact PCB label / flash marking)
2. Same-board backup exists and size matches chip capacity
3. Candidate image is validated
4. Recovery plan is recorded
5. Confirmation phrase equals the exact board identity string
6. flashrom is available

```powershell
.\target\release\bios-tool.exe write `
  --image candidate.bin `
  --backup board_backup.bin `
  --board-model "B760M AORUS" `
  --board-rev "1.1" `
  --chip-part "MX25L25673G" `
  --chip-size 33554432 `
  --operator-confirmed true `
  --confirm-write "Gigabyte B760M AORUS PCB:1.1" `
  --recovery-plan "external programmer + dual-BIOS or known-good dump" `
  --dry-run true

# Remove --dry-run only after dry-run succeeds and recovery is ready
```

Force / unlock / protection-bypass flags are **never** added automatically.

