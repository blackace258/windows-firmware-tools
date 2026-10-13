# windows-firmware-tools

Windows-native tools for SPI BIOS / firmware work.

## Packages

| Folder | Language | Description |
|--------|----------|-------------|
| **bios-tool** | Rust | CLI + professional GUI for SPI dump / inspect / validate / **safety-gated write & erase** (flashrom backend) |
| **firmware_tool** | Python | Read-only firmware image inspection, hashing, UEFI FV / descriptor parsing, comparison |
| **smbios-tool** | Rust | Read-only SMBIOS/DMI acquisition and reporting (Windows API) |

## Quick start (bios-tool)

```powershell
cd bios-tool
.\setup_windows.ps1 -Install -Build
.\target\release\bios-tool.exe doctor

# Professional GUI
cargo build --release --features gui --bin bios-tool-gui
.\target\release\bios-tool-gui.exe
```

**Safety:** All write/erase paths require exact board identity confirmation phrase, verified same-board backup, and recovery plan. No `--yes` bypass.

Full source archive is available as `windows-firmware-tools.zip`.

## License

MIT
