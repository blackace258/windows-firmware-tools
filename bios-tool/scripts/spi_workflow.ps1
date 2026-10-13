# spi_workflow.ps1
# Windows helper for SPI chip READ (dump) and gated WRITE via bios-tool + flashrom.
# Never loads a custom kernel driver. Uses flashrom as a child process only.
#
# Prerequisites:
#   1. .\setup_windows.ps1 -Install -Build   (Rust + MSVC, then cargo build)
#   2. A Windows-compatible flashrom.exe on PATH or set $env:FLASHROM_PATH
#   3. Programmer hardware + signed vendor drivers (e.g. CH341A)
#   4. $env:FLASHROM_PROGRAMMER = "ch341a_spi"   # or your programmer name
#
# Usage:
#   .\scripts\spi_workflow.ps1 -Read  -Output board_backup.bin
#   .\scripts\spi_workflow.ps1 -Write -Image candidate.bin -Backup board_backup.bin `
#        -BoardModel "B760M AORUS" -BoardRev "1.1" -ChipPart "MX25L25673G" `
#        -ConfirmWrite "Gigabyte B760M AORUS PCB:1.1" -DryRun

param(
    [switch]$Read,
    [switch]$Write,
    [switch]$Probe,
    [switch]$DryRun,

    [string]$Output = "spi_dump.bin",
    [string]$Image = "",
    [string]$Backup = "",
    [int]$Reads = 2,

    [string]$BoardManufacturer = "Gigabyte",
    [string]$BoardModel = "",
    [string]$BoardRev = "1.1",
    [string]$ChipVendor = "Macronix",
    [string]$ChipPart = "",
    [long]$ChipSize = 33554432,

    [string]$ConfirmWrite = "",
    [string]$RecoveryPlan = "external programmer + verified same-board backup"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Exe = Join-Path $Root "target\release\bios-tool.exe"
if (-not (Test-Path $Exe)) {
    $Exe = Join-Path $Root "target\debug\bios-tool.exe"
}
if (-not (Test-Path $Exe)) {
    Write-Host "bios-tool.exe not found. Run: .\setup_windows.ps1 -Build" -ForegroundColor Red
    exit 1
}

Write-Host "bios-tool: $Exe"
Write-Host "FLASHROM_PATH=$($env:FLASHROM_PATH)"
Write-Host "FLASHROM_PROGRAMMER=$($env:FLASHROM_PROGRAMMER)"
Write-Host ""

if ($Probe) {
    & $Exe device probe
    exit $LASTEXITCODE
}

if ($Read) {
    Write-Host "=== SPI READ (full-chip dump, $Reads consistent reads) ===" -ForegroundColor Cyan
    $args = @(
        "dump",
        "--output", $Output,
        "--reads", $Reads
    )
    if ($BoardModel) { $args += @("--board-model", $BoardModel) }
    if ($BoardRev) { $args += @("--board-rev", $BoardRev) }
    & $Exe @args
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        Write-Host "Dump OK: $Output" -ForegroundColor Green
        Write-Host "Keep this file as the same-board backup before any write."
    }
    exit $code
}

if ($Write) {
    if (-not $Image -or -not $Backup -or -not $BoardModel -or -not $ChipPart -or -not $ConfirmWrite) {
        Write-Host "Write requires: -Image -Backup -BoardModel -ChipPart -ConfirmWrite" -ForegroundColor Red
        exit 1
    }
    Write-Host "=== SPI WRITE (gated; flashrom -w only after safety machine) ===" -ForegroundColor Cyan
    Write-Host "Image:   $Image"
    Write-Host "Backup:  $Backup"
    Write-Host "Board:   $BoardManufacturer $BoardModel PCB:$BoardRev"
    Write-Host "Chip:    $ChipVendor $ChipPart size=$ChipSize"
    Write-Host "Confirm: $ConfirmWrite"
    Write-Host "DryRun:  $DryRun"
    Write-Host ""

    $args = @(
        "write",
        "--image", $Image,
        "--backup", $Backup,
        "--confirm-write", $ConfirmWrite,
        "--recovery-plan", $RecoveryPlan,
        "--board-manufacturer", $BoardManufacturer,
        "--board-model", $BoardModel,
        "--board-rev", $BoardRev,
        "--chip-vendor", $ChipVendor,
        "--chip-part", $ChipPart,
        "--chip-size", $ChipSize,
        "--operator-confirmed", "true"
    )
    if ($DryRun) { $args += @("--dry-run", "true") }

    & $Exe @args
    exit $LASTEXITCODE
}

Write-Host "Specify -Probe, -Read, or -Write"
Write-Host "Examples:"
Write-Host '  .\scripts\spi_workflow.ps1 -Probe'
Write-Host '  .\scripts\spi_workflow.ps1 -Read -Output board_backup.bin'
Write-Host '  .\scripts\spi_workflow.ps1 -Write -Image new.bin -Backup board_backup.bin -BoardModel "B760M AORUS" -ChipPart "MX25L25673G" -ConfirmWrite "Gigabyte B760M AORUS PCB:1.1" -DryRun'
exit 1
