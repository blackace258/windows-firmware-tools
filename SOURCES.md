# Layout

This repository is the source. There is no side archive.

```
bios-tool/          Rust SPI CLI + GUI + setup_windows.ps1
firmware_tool/      Python image inspection + SMBIOS mirror
smbios-tool/        Rust SMBIOS reader + gated vendor-tool write
skills/autonomous-windows-dev/
  SKILL.md
  references/directories.md
  references/errors.md
```

`smbios-tool` is the canonical SMBIOS implementation. `firmware_tool/smbios/` must keep the same gate order and allowed fields. Do not add a third copy.

Empty directories are not part of the tree. `bios-tool` writes audit JSON to `./logs/` at runtime; that directory is gitignored.
