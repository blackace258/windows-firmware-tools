# windows-firmware-tools

Windows tools for SPI BIOS work and published SMBIOS/DMI. The git tree is the complete source. Do not look for a zip.

| Directory | What it is |
|---|---|
| [bios-tool](bios-tool/) | Rust CLI + GUI. flashrom is a child process only. Full-chip write and erase go through the safety state machine. Dry-run returns before flashrom runs. |
| [smbios-tool](smbios-tool/) | Rust reader for the published SMBIOS table (`GetSystemFirmwareTable` RSMB). Canonical gated DMI write: external vendor tool only, after the gates. |
| [firmware_tool](firmware_tool/) | Python 3 stdlib image inspection, plus a mirror of the SMBIOS gates. No SPI programming. |
| [skills/autonomous-windows-dev](skills/autonomous-windows-dev/SKILL.md) | Agent skill. Rules live in `SKILL.md`. Directory and error checks live in `references/` so they are not repeated. |

## Safety

Write and erase paths require an exact confirmation phrase, a verified backup, and a recovery plan. There is no `--yes` bypass. Dry-run does not spawn flashrom or a vendor utility and is not reported as `Verified`.

## Build on Windows

```powershell
cd bios-tool
.\setup_windows.ps1 -Install -Build
.\target\release\bios-tool.exe doctor

cargo build --release --features gui --bin bios-tool-gui

cd ..\smbios-tool
cargo build --release
cargo test

cd ..\firmware_tool
python -m unittest discover -s tests -v
```

Live `GetSystemFirmwareTable` and flashrom need a Windows machine. Parsing, compare, and the safety gates run without hardware.

## License

MIT
