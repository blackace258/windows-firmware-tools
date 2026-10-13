# Complete sources

The GitHub tree is being populated incrementally. **The authoritative complete source archive is:**

**`windows-firmware-tools.zip`** (project artifacts / download and expand).

## Archive contents (verified)

### bios-tool/ (Rust CLI + GUI)
- Cargo.toml, Cargo.lock, README.md, .gitignore
- setup_windows.ps1, setup_windows.cmd
- src/main.rs, src/lib.rs, src/bin/gui.rs
- src/cli/mod.rs
- src/domain/{mod,board,chip,error,image,safety}.rs
- src/backend/{mod,flashrom}.rs
- src/programming/mod.rs
- src/acquisition/mod.rs, src/audit/mod.rs, src/setup/mod.rs, src/validation/mod.rs
- src/firmware/{mod,descriptor,signatures,uefi}.rs
- src/gui/mod.rs
- tests/safety_machine.rs

### firmware_tool/ (Python)
- Full package: cli, acquisition, comparison, parsing, reporting, smbios, validation, tests

### smbios-tool/ (Rust)
- Cargo.toml, README.md, src/{main,lib,error,acquisition,capability,compare,parser,report,write}, tests

## Build (Windows)

```powershell
# Expand the zip, then:
cd bios-tool
.\setup_windows.ps1 -Install -Build
cargo build --release --features gui --bin bios-tool-gui
```

Safety model is mandatory for all write/erase paths.
