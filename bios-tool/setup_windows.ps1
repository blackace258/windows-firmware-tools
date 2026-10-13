# setup_windows.ps1
# Dependency check / guided install / build helper for bios-tool on Windows 10/11 x64.
# Safe to re-run. Never elevates silently. Never flashes or writes firmware.
#
# Usage:
#   .\setup_windows.ps1                 # check + report (default)
#   .\setup_windows.ps1 -CheckOnly      # same as default
#   .\setup_windows.ps1 -Install        # check, then consented install steps
#   .\setup_windows.ps1 -Build          # check, then cargo build --release if ready
#   .\setup_windows.ps1 -Install -Build # install guidance then build when possible
#
# Exit codes:
#   0 = all required deps present (and build succeeded if -Build)
#   1 = required deps missing or build failed

param(
    [switch]$CheckOnly,
    [switch]$Install,
    [switch]$Build
)

$ErrorActionPreference = "Continue"
$ScriptRoot = $PSScriptRoot
$LogDir = Join-Path $ScriptRoot "logs"
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir | Out-Null
}
$LogFile = Join-Path $LogDir ("setup_" + (Get-Date -Format "yyyyMMdd_HHmmss") + ".log")

function Write-Log {
    param([string]$msg)
    $line = "$(Get-Date -Format o) $msg"
    Add-Content -Path $LogFile -Value $line
    Write-Host $msg
}

function Write-StatusTable {
    param($Items)
    Write-Host ""
    Write-Host ("{0,-38} {1,-12} {2}" -f "Dependency", "Status", "Detail")
    Write-Host ("-" * 95)
    foreach ($i in $Items) {
        $color = switch ($i.Status) {
            "Found"    { "Green" }
            "Missing"  { "Red" }
            "Optional" { "Yellow" }
            default    { "White" }
        }
        Write-Host ("{0,-38} {1,-12} {2}" -f $i.Name, $i.Status, $i.Detail) -ForegroundColor $color
    }
    Write-Host ""
}

function Test-CommandExists {
    param([string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-CommandVersion {
    param([string]$Name, [string[]]$Args = @("--version"))
    try {
        $out = & $Name @Args 2>$null
        if ($LASTEXITCODE -eq 0 -or $out) {
            return ($out | Out-String).Trim()
        }
    } catch {}
    return $null
}

# ---------------------------------------------------------------------------
# Collect status
# ---------------------------------------------------------------------------
$items = @()
$requiredOk = $true

Write-Log "=== bios-tool Windows setup ==="
Write-Log "Script: $PSCommandPath"
Write-Log "Log: $LogFile"

# Windows
try {
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $arch = $env:PROCESSOR_ARCHITECTURE
    $winDetail = "$($os.Caption) Build $($os.BuildNumber) $arch"
} catch {
    $winDetail = "$env:OS $env:PROCESSOR_ARCHITECTURE"
}
$items += [pscustomobject]@{ Name = "Windows edition/build"; Status = "Found"; Detail = $winDetail }
Write-Log "Windows: $winDetail"

# Elevation
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$items += [pscustomobject]@{
    Name = "Elevated shell"
    Status = "Optional"
    Detail = if ($elevated) { "Yes (admin)" } else { "No (ok for inspect / build)" }
}

# rustc
$rustcVer = Get-CommandVersion "rustc"
if ($rustcVer) {
    $items += [pscustomobject]@{ Name = "rustc"; Status = "Found"; Detail = $rustcVer }
} else {
    $items += [pscustomobject]@{ Name = "rustc"; Status = "Missing"; Detail = "Install via rustup (https://rustup.rs)" }
    $requiredOk = $false
}

# cargo
$cargoVer = Get-CommandVersion "cargo"
if ($cargoVer) {
    $items += [pscustomobject]@{ Name = "cargo"; Status = "Found"; Detail = $cargoVer }
} else {
    $items += [pscustomobject]@{ Name = "cargo"; Status = "Missing"; Detail = "Comes with rustup" }
    $requiredOk = $false
}

# MSVC linker (link.exe) — required for windows-msvc target
$linkFound = $false
$linkDetail = "not found"
if (Test-CommandExists "link") {
    $linkFound = $true
    $linkDetail = "link.exe in PATH"
} else {
    $vsPaths = @(
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\BuildTools\VC\Tools\MSVC"
    )
    foreach ($base in $vsPaths) {
        if (Test-Path $base) {
            $candidate = Get-ChildItem -Path $base -Recurse -Filter "link.exe" -ErrorAction SilentlyContinue |
                Select-Object -First 1
            if ($candidate) {
                $linkFound = $true
                $linkDetail = $candidate.FullName
                break
            }
        }
    }
}
if ($linkFound) {
    $items += [pscustomobject]@{ Name = "MSVC linker (link.exe)"; Status = "Found"; Detail = $linkDetail }
} else {
    $items += [pscustomobject]@{
        Name = "MSVC linker (link.exe)"
        Status = "Missing"
        Detail = "Install VS Build Tools + 'Desktop development with C++'"
    }
    $requiredOk = $false
}

# Windows SDK (optional but useful)
$sdk = $null
$sdkRoots = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\Include",
    "${env:ProgramFiles}\Windows Kits\10\Include"
)
foreach ($r in $sdkRoots) {
    if (Test-Path $r) {
        $sdk = (Get-ChildItem $r -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1).Name
        if ($sdk) { break }
    }
}
if ($sdk) {
    $items += [pscustomobject]@{ Name = "Windows SDK"; Status = "Found"; Detail = "10.$sdk (or similar)" }
} else {
    $items += [pscustomobject]@{ Name = "Windows SDK"; Status = "Optional"; Detail = "usually installed with Build Tools" }
}

# flashrom (optional for pure offline inspect)
$flashromVer = $null
if ($env:FLASHROM_PATH -and (Test-Path $env:FLASHROM_PATH)) {
    $flashromVer = "FLASHROM_PATH=$($env:FLASHROM_PATH)"
} else {
    $flashromVer = Get-CommandVersion "flashrom"
}
if ($flashromVer) {
    $items += [pscustomobject]@{ Name = "flashrom"; Status = "Found"; Detail = $flashromVer }
} else {
    $items += [pscustomobject]@{
        Name = "flashrom"
        Status = "Optional"
        Detail = "not in PATH; set FLASHROM_PATH for hardware ops"
    }
}

# winget
$winget = Get-Command winget -ErrorAction SilentlyContinue
if ($winget) {
    $items += [pscustomobject]@{ Name = "winget"; Status = "Optional"; Detail = "available for consented installs" }
} else {
    $items += [pscustomobject]@{ Name = "winget"; Status = "Optional"; Detail = "not found" }
}

# Git (helpful for cargo)
$gitVer = Get-CommandVersion "git"
if ($gitVer) {
    $items += [pscustomobject]@{ Name = "git"; Status = "Found"; Detail = ($gitVer -split "`n")[0] }
} else {
    $items += [pscustomobject]@{ Name = "git"; Status = "Optional"; Detail = "recommended for cargo" }
}

Write-StatusTable $items
Write-Log "requiredOk=$requiredOk"

# ---------------------------------------------------------------------------
# Default / CheckOnly: stop after report
# ---------------------------------------------------------------------------
if ($CheckOnly -or (-not $Install -and -not $Build)) {
    Write-Host "Mode: check-only."
    Write-Host "  Re-run with -Install  for guided dependency installation."
    Write-Host "  Re-run with -Build    to compile bios-tool after deps are ready."
    Write-Host "  Re-run with -Install -Build for both."
    if ($requiredOk) {
        Write-Log "All required dependencies present."
        exit 0
    } else {
        Write-Log "Required dependencies missing."
        exit 1
    }
}

# ---------------------------------------------------------------------------
# Install path (explicit consent only)
# ---------------------------------------------------------------------------
if ($Install) {
    Write-Host ""
    Write-Host "=== Install mode (consent required for every action) ===" -ForegroundColor Cyan
    Write-Host "No silent elevation. No unsigned drivers. No firmware flash."
    Write-Host ""

    # Rust via rustup
    if (-not $rustcVer) {
        Write-Host "Rust toolchain is missing."
        Write-Host "Official source: https://rustup.rs"
        $ans = Read-Host "Open rustup.rs in browser and/or download rustup-init.exe? (yes/no)"
        if ($ans -eq "yes") {
            Write-Log "User consented to open rustup.rs"
            Start-Process "https://rustup.rs"
            if ($winget) {
                Write-Host ""
                Write-Host "Optional: winget can install Rust. Package id is usually 'Rustlang.Rustup'."
                $w = Read-Host "Run 'winget install Rustlang.Rustup' now? (yes/no)"
                if ($w -eq "yes") {
                    Write-Log "User consented to winget install Rustlang.Rustup"
                    winget install --id Rustlang.Rustup -e --accept-source-agreements
                    Write-Host "After install, close and re-open this PowerShell, then re-run the script."
                }
            } else {
                Write-Host "Download and run rustup-init.exe from the opened page, then re-open PowerShell."
            }
        }
    } else {
        Write-Host "Rust already present: $rustcVer"
    }

    # MSVC Build Tools
    if (-not $linkFound) {
        Write-Host ""
        Write-Host "MSVC Build Tools (link.exe) are required for the windows-msvc target."
        Write-Host "Official installer: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
        Write-Host "In the installer select workload: 'Desktop development with C++'."
        $ans = Read-Host "Open Visual Studio Build Tools page? (yes/no)"
        if ($ans -eq "yes") {
            Write-Log "User consented to open VS Build Tools page"
            Start-Process "https://visualstudio.microsoft.com/visual-cpp-build-tools/"
        }
        if ($winget) {
            Write-Host ""
            Write-Host "Optional: winget can install Build Tools."
            Write-Host "Package id example: Microsoft.VisualStudio.2022.BuildTools"
            $w = Read-Host "Attempt 'winget install Microsoft.VisualStudio.2022.BuildTools' ? (yes/no)"
            if ($w -eq "yes") {
                Write-Log "User consented to winget install Build Tools"
                winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements
                Write-Host "After install, open a NEW 'x64 Native Tools' or Developer PowerShell and re-run this script."
            }
        }
    } else {
        Write-Host "MSVC linker already present."
    }

    # flashrom guidance only
    if (-not $flashromVer) {
        Write-Host ""
        Write-Host "flashrom (optional for hardware dump/write):"
        Write-Host "  Only use a build you have verified (hash/signature) against official/reputable sources."
        Write-Host "  Place the binary somewhere permanent and set:"
        Write-Host '    $env:FLASHROM_PATH = "C:\path\to\flashrom.exe"'
        Write-Host "  Never auto-install unsigned USB programmer drivers."
    }

    Write-Log "Install guidance finished."
    Write-Host ""
    Write-Host "Re-run this script (or open a fresh shell) after installing tools so PATH is refreshed."
}

# ---------------------------------------------------------------------------
# Build path
# ---------------------------------------------------------------------------
if ($Build) {
    Write-Host ""
    Write-Host "=== Build mode ===" -ForegroundColor Cyan

    $rustcNow = Get-CommandVersion "rustc"
    $cargoNow = Get-CommandVersion "cargo"
    if (-not $rustcNow -or -not $cargoNow) {
        Write-Host "Cannot build: rustc/cargo still missing." -ForegroundColor Red
        Write-Host "Install Rust, open a new PowerShell, then re-run with -Build."
        Write-Log "Build aborted: missing Rust"
        exit 1
    }

    $linkNow = Test-CommandExists "link"
    if (-not $linkNow) {
        Write-Host "WARNING: link.exe not in current PATH." -ForegroundColor Yellow
        Write-Host "If cargo fails, open 'x64 Native Tools Command Prompt for VS' or"
        Write-Host "Developer PowerShell for VS, then re-run this script with -Build."
    }

    Push-Location $ScriptRoot
    try {
        Write-Log "Running: cargo build --release"
        Write-Host "Running: cargo build --release ..."
        & cargo build --release
        $buildExit = $LASTEXITCODE
        if ($buildExit -eq 0) {
            $exe = Join-Path $ScriptRoot "target\release\bios-tool.exe"
            if (Test-Path $exe) {
                Write-Host ""
                Write-Host "Build succeeded." -ForegroundColor Green
                Write-Host "Binary: $exe"
                Write-Log "Build succeeded: $exe"
            } else {
                Write-Host "cargo reported success but bios-tool.exe not found at expected path." -ForegroundColor Yellow
                Write-Log "Build exit 0 but exe missing"
            }
            exit 0
        } else {
            Write-Host "cargo build failed (exit $buildExit)." -ForegroundColor Red
            Write-Log "Build failed exit=$buildExit"
            exit 1
        }
    } finally {
        Pop-Location
    }
}

if ($requiredOk) { exit 0 } else { exit 1 }
