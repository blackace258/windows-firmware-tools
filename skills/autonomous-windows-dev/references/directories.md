# Directories

One tree per tool. Do not invent a sibling project that repeats the same commands.

## This workspace

Persistent project files live under `/workspace/artifacts/`. There is no `/home/workdir/artifacts/` tree. The app-builder app in `/workspace/src` is unrelated; do not put Windows tools there.

| Path | Role | Do not duplicate |
|---|---|---|
| `artifacts/bios-tool/` | Rust SPI CLI + GUI. flashrom child process only. Safety-gated full-chip write and erase. | A second SPI programmer or kernel driver |
| `artifacts/smbios-tool/` | Rust published-SMBIOS reader and gated vendor-tool DMI write | Another Rust SMBIOS crate |
| `artifacts/firmware_tool/` | Python stdlib image inspection plus a Python mirror of the SMBIOS gates | A third parser for the same image formats |

`smbios-tool` is the canonical SMBIOS implementation. `firmware_tool/smbios/` is the stdlib mirror for machines without Rust. Keep their gate order and allowed fields the same. Do not add SPI flashing to the Python tool.

## Layout rules

- Match the folder that already exists (`src/<module>/mod.rs` in the Rust crates, package directories in `firmware_tool`). Do not add `app/`, `lib/`, or a new top-level language.
- No empty directories. Runtime output (`bios-tool/logs/`) is created by the program and gitignored, not committed as an empty folder.
- Do not commit `target/`, `__pycache__/`, or `*.pyc`.
- A GitHub tree must contain the source itself. Do not point the README at a zip that is not in the repository, and do not leave a partial tree next to a "full sources are elsewhere" note.
- Windows scripts stay `.ps1` / `.cmd` with paths the script actually uses. Do not add a Unix-only entry point as the only way to build.
