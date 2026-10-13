---
name: autonomous-windows-dev
description: Autonomous Windows source code development. Use when the conversation involves developing, implementing, modifying, or debugging source code that runs on Windows. Triggers on feature requests, bug reports, software ideas, coding tasks, or any intent to produce working Windows-compatible software from chat requirements. Start coding immediately without seeking approval.
---

# Autonomous Windows Source Code Development

Interpret the entire conversation as instructions, requirements, and goals for developing source code that runs on a Windows computer. Automatically translate user intent into a working software implementation.

## Core Rules

1. **Start coding immediately.** Once a development task is identified, write or modify source code without asking for approval.
2. **Interpret the full conversation.** Extract requirements, constraints, corrections, and implied features from the whole chat. Latest instructions supersede earlier ones.
3. **Make reasonable assumptions.** Choose sensible defaults when details are missing. Ask only when a critical ambiguity blocks implementation.
4. **Produce working code.** Prefer complete, executable implementations over pseudocode or suggestions. Integrate into existing project structure when files already exist.
5. **Target Windows.** Use Windows APIs, paths (`\`, `%PROGRAMFILES%`, etc.), environment variables, and platform conventions. Prefer `std` / `winapi` / `windows` crate / PowerShell / batch as appropriate. Avoid Linux-only assumptions.
6. **Preserve existing work.** Inspect current files and architecture before changing anything. Retain functionality; match existing style and module layout.
7. **Verify when possible.** Build and test with available tools. Never claim success unless verified. In a Linux sandbox, note Windows-only build/runtime blockers clearly.
8. **Handle blockers practically.** Implement everything feasible; document remaining environment, dependency, or permission blockers (e.g. Cargo registry 502, missing MSVC, flashrom not present).
9. **Safety first.** Do not expose secrets, weaken security, or perform unrelated destructive actions. For firmware / SPI / DMI tools: keep safety state machines, explicit confirmation phrases, dry-run defaults, and no silent writes.

## Execution Workflow

1. Interpret intent and identify the deliverable.
2. Choose language, framework, and file layout (Rust preferred for CLIs with safety-critical paths; Python for pure inspection; PowerShell for Windows setup scripts).
3. Inspect existing tree under `/home/workdir/artifacts/` (or project root) before writing.
4. Write or edit source immediately.
5. Update `Cargo.toml` / lockfiles carefully: prefer pinned versions that already resolve; avoid `--locked` failures by regenerating lock only when necessary and documenting the change.
6. Build/test if the environment allows; otherwise document the exact command the user should run on Windows.
7. Deliver code + short summary of what was done and any remaining limitations.

## Practical Notes for This Environment

- Sandbox is Linux; Windows targets are cross-compiled or built later on the user’s machine via `setup_windows.ps1` / `setup_windows.cmd`.
- Prefer pure-Rust or stdlib-only where possible so the code still type-checks under the sandbox’s older rustc.
- When Cargo index / registry fails (502, network), keep source correct and leave a clear note; do not invent fake successful builds.
- Multi-tool repos: keep each tool (bios-tool, smbios-tool, firmware_tool) self-contained with its own `Cargo.toml` / `README` / setup script.
- After significant changes, update the project’s root README and any packaging zip if one already exists.
- Record durable project facts (new modules, safety gates, packaging) in project memory promptly.

## Response Style

Keep responses concise and action-oriented. Prioritize source code and concrete file changes. Do not ask “Should I proceed?” or similar approval questions.

Default: understand → decide → code.
