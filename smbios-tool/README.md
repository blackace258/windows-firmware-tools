# smbios-tool

Windows-native utility for the published SMBIOS/DMI table: read always; **gated write** only via an operator-supplied vendor utility after the safety state machine passes.

Implements the Capstone from the *DMI/SMBIOS Engineering — Complete Study Course*:

- Acquire the raw table via `GetSystemFirmwareTable` (RSMB provider)
- Validate and parse structures with checked bounds and string-set handling
- Report Type 0 (BIOS), Type 1 (System), Type 2 (Baseboard), Type 3 (Enclosure)
- Preserve raw bytes and SHA-256 for reproducibility
- Compare two captures and summarize field differences
- Emit machine-readable JSON
- Clearly distinguish **published SMBIOS values** from any **persistent firmware source**
- Optional gated write workflow (Lesson 11 safety state machine)

## Core principle

> Reading a published SMBIOS table, changing a local copy, and changing the firmware-managed source of the data are distinct operations.

This tool reads the published table. Persistent writes are never done by patching the published table or by blind SPI edits. A write, if authorized, only invokes a vendor utility you supply and verify.

## Build (Windows x64)

```powershell
cargo build --release
cargo test
```

Requires a Windows host for live `GetSystemFirmwareTable`. On non-Windows, use `--from-file` with a previously captured table.

## Commands

```text
smbios-tool read [--raw-out table.bin] [--json-out report.json]
smbios-tool inspect [--from-file table.bin] [--json-out report.json]
smbios-tool compare capture_a.bin capture_b.bin [--json-out diff.json]
smbios-tool capability [--profile gigabyte-b760m-aorus-rev1.1] [--json-out cap.json]

smbios-tool write
  --board-model "..." --board-rev "..."
  --field type1.serial_number --value "NEWSERIAL"
  --backup smbios_before.bin
  --recovery-plan "external programmer + known-good dump"
  --confirm-write "I AUTHORIZE DMI WRITE ON <exact identity>"
  [--vendor-tool path\to\utility.exe --vendor-arg ...]
  [--vendor-sha256 <hex>]
  [--dry-run]
  [--audit-out audit.json]
```

### Read workflow

```powershell
smbios-tool read --raw-out smbios_before.bin --json-out before.json
smbios-tool read --raw-out smbios_after.bin --json-out after.json
smbios-tool compare smbios_before.bin smbios_after.bin --json-out diff.json
smbios-tool capability
```

### Gated write workflow (fail-closed)

Safety states:

```text
Unidentified → Identified → BackupVerified → InputValidated
→ RecoveryConfirmed → WriteAuthorized → Executing → Verified
Any failure → Blocked

Dry-run stops at `WriteAuthorized`. It does not spawn a vendor tool and is not `Verified`.
```

```powershell
# 1. Capture baseline on the physical machine
smbios-tool read --raw-out smbios_before.bin

# 2. Dry-run the full gate sequence (no vendor tool spawned)
smbios-tool write `
  --board-model "Gigabyte B760M AORUS" `
  --board-rev "1.1" `
  --field type1.serial_number --value "TEST-SERIAL" `
  --backup smbios_before.bin `
  --recovery-plan "external programmer + known-good dump on dual-BIOS or CH341A" `
  --confirm-write "I AUTHORIZE DMI WRITE ON Gigabyte B760M AORUS PCB:1.1" `
  --dry-run `
  --audit-out write_dryrun.json

# 3. Real execute only with a verified vendor utility you provide
smbios-tool write `
  --board-model "Gigabyte B760M AORUS" `
  --board-rev "1.1" `
  --field type1.serial_number --value "PROD-SERIAL" `
  --backup smbios_before.bin `
  --recovery-plan "external programmer + known-good dump" `
  --confirm-write "I AUTHORIZE DMI WRITE ON Gigabyte B760M AORUS PCB:1.1" `
  --vendor-tool "C:\trusted\AMIDEWINx64.EXE" `
  --vendor-arg /SS --vendor-arg "PROD-SERIAL" `
  --vendor-sha256 <sha256-of-that-binary> `
  --audit-out write_audit.json

# 4. Prove persistence independently (reboot recommended)
smbios-tool read --raw-out smbios_after.bin
smbios-tool compare smbios_before.bin smbios_after.bin
```

Allowed `--field` keys:

- `type1.serial_number`, `type1.uuid`, `type1.product_name`, `type1.manufacturer`
- `type2.serial_number`, `type2.product`, `type2.manufacturer`
- `type3.serial_number`, `type3.asset_tag`

## Safety / limits

- Malformed tables produce errors, not panics.
- String indexes are validated; index 0 means “no string”.
- Unknown structure types are preserved, not rejected.
- **Write path is fail-closed**: without `--dry-run`, a real `--vendor-tool` path and non-empty `--vendor-arg` values are required.
- This project does **not** embed AMIDEWIN, does **not** load unsigned drivers, and does **not** perform SPI byte-patching for DMI strings.
- A successful vendor-tool exit code does **not** prove persistence — always re-read after reboot.
- SPI full-chip programming remains the separate **bios-tool** project.

## Relationship to bios-tool

| Concern | Tool |
|---------|------|
| Published SMBIOS table (Windows API) + gated vendor DMI write | **smbios-tool** (this project) |
| SPI flash dump / inspect / gated program | **bios-tool** (`artifacts/bios-tool`) |

Both share the same target board context (Gigabyte B760M AORUS family, PCB rev 1.1 provisional, MX25L25673G-class flash) and the same safety mindset: confirm identity, preserve backups, fail closed.

## License

MIT
