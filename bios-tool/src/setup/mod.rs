//! Dependency checks and diagnostics. System installation is owned by setup_windows.ps1.

use crate::domain::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,
    pub status: StatusKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatusKind {
    Found,
    Missing,
    Optional,
    NeedsConsent,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub windows_info: String,
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub msvc_linker: bool,
    pub flashrom: Option<String>,
    pub programmer_note: String,
    pub elevated: bool,
    pub items: Vec<DependencyStatus>,
}

pub fn run_doctor(programmer: &str) -> Result<DoctorReport> {
    let mut items = Vec::new();

    let windows_info = detect_windows_info();
    items.push(DependencyStatus {
        name: "Windows".into(),
        status: StatusKind::Found,
        detail: windows_info.clone(),
    });

    let rustc = run_capture("rustc", &["--version"]);
    items.push(DependencyStatus {
        name: "rustc".into(),
        status: if rustc.is_some() { StatusKind::Found } else { StatusKind::Missing },
        detail: rustc.clone().unwrap_or_else(|| "not found".into()),
    });

    let cargo = run_capture("cargo", &["--version"]);
    items.push(DependencyStatus {
        name: "cargo".into(),
        status: if cargo.is_some() { StatusKind::Found } else { StatusKind::Missing },
        detail: cargo.clone().unwrap_or_else(|| "not found".into()),
    });

    let msvc = detect_msvc_linker();
    items.push(DependencyStatus {
        name: "MSVC linker (link.exe)".into(),
        status: if msvc { StatusKind::Found } else { StatusKind::Missing },
        detail: if msvc {
            "link.exe found in PATH".into()
        } else {
            "MSVC Build Tools / Windows SDK required for *-pc-windows-msvc target".into()
        },
    });

    let flashrom = run_capture("flashrom", &["--version"]);
    items.push(DependencyStatus {
        name: "flashrom".into(),
        status: if flashrom.is_some() { StatusKind::Found } else { StatusKind::Missing },
        detail: flashrom.clone().unwrap_or_else(|| "not found in PATH; provide FLASHROM_PATH or install a verified Windows build".into()),
    });

    let backend = crate::backend::FlashromBackend::from_env_or_default();
    let note = backend.programmer_windows_support_note().to_string();
    items.push(DependencyStatus {
        name: format!("programmer ({})", programmer),
        status: StatusKind::Optional, // actual support is environment-dependent
        detail: note.clone(),
    });

    let elevated = is_elevated();
    items.push(DependencyStatus {
        name: "elevated shell".into(),
        status: StatusKind::Optional,
        detail: if elevated { "yes (admin)".into() } else { "no".into() },
    });

    Ok(DoctorReport {
        windows_info,
        rustc,
        cargo,
        msvc_linker: msvc,
        flashrom,
        programmer_note: note,
        elevated,
        items,
    })
}

fn run_capture(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn detect_windows_info() -> String {
    #[cfg(windows)]
    {
        // Prefer environment or simple registry-free detection
        std::env::var("OS").unwrap_or_else(|_| "Windows".into())
            + " "
            + &std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        format!("non-Windows host (build target should be Windows x64); uname-like: {}",
            run_capture("uname", &["-a"]).unwrap_or_else(|| "unknown".into()))
    }
}

fn detect_msvc_linker() -> bool {
    run_capture("link", &[]).is_some() || which_exists("link.exe")
}

fn which_exists(name: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(name).exists() {
                return true;
            }
        }
    }
    false
}

fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        // Simple heuristic; real check would use Windows APIs
        std::env::var("USERNAME").map(|u| u.eq_ignore_ascii_case("SYSTEM")).unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        unsafe { libc::geteuid() == 0 }
    }
}

// libc only for non-Windows elevated check; avoid hard dep on Windows
#[cfg(not(windows))]
mod libc {
    extern "C" {
        pub fn geteuid() -> u32;
    }
}
