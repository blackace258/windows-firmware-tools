# Firmware Inspection Tool

Safe utility for inspecting motherboard BIOS/UEFI firmware images on Windows (and other platforms).

Firmware **image** commands are read-only: no flashrom, no erase, no SPI patching.

`smbios-write` is a separate fail-closed gate. It never edits an image. A real run only starts an operator-supplied vendor utility after identity, backup, recovery plan, and an exact confirmation phrase. Dry-run does not spawn that utility and is not reported as verified.

## Features

- Read-only file inspection
- SHA-256 hashing
- Size / capacity checks
- Conservative image type detection
- Binary comparison with differing-byte ranges
- Bounded UEFI Firmware Volume parsing
- Human-readable and JSON reports
- Stable finding codes and exit codes
- SMBIOS / DMI published-table reader (Windows `GetSystemFirmwareTable(RSMB)` or offline file)
- Safe structure parser (Type 0/1/2/3), capture compare, capability report
- Gated DMI write via an external vendor tool only (`smbios-write`)

## Requirements

- Python 3.9+ (tested with 3.10+)
- Standard library only — no external dependencies

## Usage (Windows)

Open a Command Prompt or PowerShell in the `firmware_tool` directory:

```bat
python cli.py inspect path\to\firmware.bin
python cli.py inspect path\to\firmware.bin --format json
python cli.py hash path\to\firmware.bin
python cli.py compare dump1.bin dump2.bin --byte-ranges
python cli.py parse-uefi path\to\firmware.bin --output report.json
python cli.py identify-package path\to\firmware.bin
python cli.py report path\to\firmware.bin --format text

REM SMBIOS published table (live on Windows, or from saved raw file)
python cli.py smbios-read
python cli.py smbios-read --save-raw smbios_raw.bin --output smbios.json
python cli.py smbios-read --file smbios_raw.bin --format json
python cli.py smbios-compare capture_a.bin capture_b.bin
python cli.py smbios-capability --board-model "Gigabyte B760M AORUS" --board-rev "1.1"

REM Gated write. Dry-run plans only. Omit --dry-run only with a real vendor tool.
python cli.py smbios-write --board-model "Gigabyte B760M AORUS" --board-rev "1.1" --field type1.serial_number --value TEST --backup smbios_raw.bin --recovery-plan "external programmer and known-good dump" --confirm-write "I AUTHORIZE DMI WRITE ON Gigabyte B760M AORUS PCB:1.1" --dry-run
```

### Optional capacity check

```bat
python cli.py inspect image.bin --capacity 16777216
```

(16777216 = 16 MiB = 128 Mbit)

## Exit codes

| Code | Meaning                                      |
|------|----------------------------------------------|
| 0    | Success, checks passed                       |
| 1    | Operation failed                             |
| 2    | Invalid or malformed input                   |
| 3    | Completed with warnings / unresolved checks  |
| 4    | Hardware / programmer error (future)         |

## Safety

- Image commands never modify the source file and never talk to a programmer.
- Parsers validate lengths and reject out-of-bounds structures.
- No code from the firmware image is ever executed.
- `smbios-write` fails closed. Dry-run stays at `WriteAuthorized`, not `Verified`.

## Running tests

```bat
cd firmware_tool
python -m unittest discover -s tests -v
```

## Project layout

```
firmware_tool/
  cli.py
  models.py
  acquisition/
  validation/
  parsing/
  comparison/
  reporting/
  smbios/          # reader + gated write mirror of smbios-tool
  tests/
```

## Roadmap

- Package / region recognition improvements
- Optional read-only flashrom adapter lives in bios-tool, not here
- SPI programming lives in bios-tool, not here

## References

- UEFI PI Spec: https://uefi.org/specs/PI/1.10/
- flashrom: https://flashrom.org/
- UEFITool: https://github.com/LongSoft/UEFITool
