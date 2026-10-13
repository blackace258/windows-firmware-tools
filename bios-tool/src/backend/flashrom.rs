//! Windows-compatible flashrom integration via std::process::Command.
//! Verify programmer support on Windows against official docs; report unsupported clearly.

use crate::domain::{BiosToolError, Result, ChipIdentity, ChipConfidence};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashromConfig {
    pub executable: PathBuf,
    pub programmer: String,
    pub extra_args: Vec<String>,
}

impl Default for FlashromConfig {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("flashrom"),
            programmer: "internal".into(), // common; may be unsupported on many Windows setups
            extra_args: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub args: Vec<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlashromBackend {
    pub config: FlashromConfig,
}

impl FlashromBackend {
    pub fn new(config: FlashromConfig) -> Self {
        Self { config }
    }

    pub fn from_env_or_default() -> Self {
        let exe = std::env::var("FLASHROM_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("flashrom"));
        let programmer = std::env::var("FLASHROM_PROGRAMMER").unwrap_or_else(|_| "internal".into());
        Self::new(FlashromConfig {
            executable: exe,
            programmer,
            extra_args: vec![],
        })
    }

    /// Probe / identify without writing. Read-only.
    pub fn probe(&self) -> Result<(ChipIdentity, BackendResult)> {
        let mut args = vec![
            "-p".to_string(),
            self.config.programmer.clone(),
            "--verbose".to_string(),
        ];
        args.extend(self.config.extra_args.clone());

        let result = self.run(&args)?;
        let chip = parse_probe_output(&result.stdout, &result.stderr);
        Ok((chip, result))
    }

    /// Read full chip to path. Multiple reads handled by acquisition layer.
    pub fn read_chip(&self, output: &Path) -> Result<BackendResult> {
        let mut args = vec![
            "-p".to_string(),
            self.config.programmer.clone(),
            "-r".to_string(),
            output.display().to_string(),
        ];
        args.extend(self.config.extra_args.clone());
        self.run(&args)
    }

    /// Write image. Caller must have already passed safety state machine.
    pub fn write_chip(&self, image: &Path) -> Result<BackendResult> {
        let mut args = vec![
            "-p".to_string(),
            self.config.programmer.clone(),
            "-w".to_string(),
            image.display().to_string(),
        ];
        // Never auto-add -f / force / unlock / protection-bypass.
        args.extend(self.config.extra_args.clone());
        self.run(&args)
    }

    /// Verify image against chip (read-back compare by flashrom if supported).
    pub fn verify_chip(&self, image: &Path) -> Result<BackendResult> {
        let mut args = vec![
            "-p".to_string(),
            self.config.programmer.clone(),
            "-v".to_string(),
            image.display().to_string(),
        ];
        args.extend(self.config.extra_args.clone());
        self.run(&args)
    }

    /// Full-chip erase. Caller must have already passed the safety state machine.
    /// Never auto-adds force / unlock flags.
    pub fn erase_chip(&self) -> Result<BackendResult> {
        let mut args = vec![
            "-p".to_string(),
            self.config.programmer.clone(),
            "-E".to_string(),
        ];
        args.extend(self.config.extra_args.clone());
        self.run(&args)
    }

    pub fn version(&self) -> Result<String> {
        let result = self.run(&["--version".to_string()])?;
        Ok(result.stdout.lines().next().unwrap_or("unknown").to_string())
    }

    fn run(&self, args: &[String]) -> Result<BackendResult> {
        // Only hard-fail when an absolute/relative path was supplied and does not exist.
        // A bare "flashrom" is left to PATH resolution by the OS.
        let path = &self.config.executable;
        if path.as_os_str() != "flashrom" && !path.exists() {
            return Err(BiosToolError::BackendUnsupported(format!(
                "flashrom executable not found at {:?}",
                path
            )));
        }

        let mut cmd = Command::new(&self.config.executable);
        cmd.args(args);
        // Never use shell.

        let output = cmd.output().map_err(|e| {
            BiosToolError::Process(format!(
                "failed to spawn {:?}: {}. On Windows ensure a supported flashrom build and programmer drivers are present.",
                self.config.executable, e
            ))
        })?;

        let result = BackendResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            args: args.to_vec(),
            version: None,
        };

        if result.exit_code != 0 {
            // Detect common protection / unsupported messages
            let combined = format!("{}{}", result.stdout, result.stderr).to_lowercase();
            if combined.contains("protected")
                || combined.contains("write protect")
                || combined.contains("not supported")
                || combined.contains("unsupported")
            {
                return Err(BiosToolError::BackendProtection(format!(
                    "exit {}: {}",
                    result.exit_code, result.stderr
                )));
            }
            return Err(BiosToolError::Process(format!(
                "flashrom exit {}: {}",
                result.exit_code, result.stderr
            )));
        }

        Ok(result)
    }

    /// Report whether the configured programmer is known to have Windows support.
    /// This is static knowledge; real support depends on the exact flashrom build.
    pub fn programmer_windows_support_note(&self) -> &'static str {
        match self.config.programmer.as_str() {
            "internal" => "internal programmer support on Windows is limited and board/chipset dependent; many systems require Linux or a dedicated external programmer.",
            "ch341a_spi" | "ch341a" => "CH341A is commonly used on Windows with community builds; verify driver signature and flashrom Windows binary compatibility.",
            "ft2232_spi" => "FT2232-based programmers may work with appropriate drivers; confirm against the flashrom release notes for Windows.",
            _ => "Programmer Windows support is not verified for this configuration. Consult the official flashrom documentation for the selected release.",
        }
    }
}

fn parse_probe_output(stdout: &str, stderr: &str) -> ChipIdentity {
    let combined = format!("{}\n{}", stdout, stderr);
    // Very conservative parsing; real flashrom output varies by version.
    let mut vendor = "Unknown".to_string();
    let mut part = "Unknown".to_string();
    let mut capacity = 0u64;
    let mut confidence = ChipConfidence::Unknown;

    for line in combined.lines() {
        let lower = line.to_lowercase();
        if lower.contains("found") && lower.contains("flash") {
            confidence = ChipConfidence::Probed;
        }
        // Example patterns: "Macronix MX25L25673G" etc.
        if lower.contains("mx25l256") {
            vendor = "Macronix".into();
            part = "MX25L25673G".into();
            capacity = 32 * 1024 * 1024;
            confidence = ChipConfidence::Probed;
        }
        if lower.contains("size") && lower.contains("kb") {
            // crude
        }
    }

    if capacity == 0 {
        capacity = 32 * 1024 * 1024; // do not assume in production path; caller must treat Unknown carefully
        confidence = ChipConfidence::Unknown;
    }

    ChipIdentity {
        vendor,
        part_number: part,
        capacity_bytes: capacity,
        voltage_class: Some("3V".into()),
        confidence,
        probe_raw: Some(combined.chars().take(2000).collect()),
    }
}

