//! Command parsing and output. All write paths go exclusively through the safety coordinator.

use clap::{Parser, Subcommand};
use crate::domain::{
    Result, BiosToolError, BoardIdentity, ChipIdentity, IdentityConfidence, ChipConfidence,
};
use crate::backend::FlashromBackend;
use crate::setup;
use crate::acquisition;
use crate::firmware;
use crate::validation;
use crate::programming::WriteCoordinator;
use crate::audit::AuditRecord;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "bios-tool")]
#[command(about = "Windows-native SPI BIOS extraction, inspection, verification, and controlled programming", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Dependency and environment checks (no writes)
    Doctor {
        #[arg(long, default_value = "internal")]
        programmer: String,
    },

    /// Read-only backend identification
    #[command(subcommand)]
    Device(DeviceCmd),

    /// Full-chip dump with repeated reads
    Dump {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 2)]
        reads: u32,
        #[arg(long, default_value = "false")]
        force: bool,
        #[arg(long, default_value = "false")]
        dry_run: bool,
        #[arg(long, default_value = "Gigabyte B760M AORUS (provisional)")]
        board_model: String,
        #[arg(long, default_value = "1.1")]
        board_rev: String,
    },

    /// Read-only inspection of an image
    Inspect {
        image: PathBuf,
    },

    /// Compare two images
    Compare {
        a: PathBuf,
        b: PathBuf,
    },

    /// Validate image against board profile
    Validate {
        #[arg(long)]
        image: PathBuf,
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Read and compare physical chip with candidate (never writes)
    Verify {
        #[arg(long)]
        image: PathBuf,
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },

    /// Safety-gated full-chip write (requires exact confirmation phrase + operator-confirmed identities)
    Write {
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        backup: PathBuf,
        /// Exact board identity string that must match --board-* flags (e.g. "Gigabyte B760M AORUS PCB:1.1")
        #[arg(long)]
        confirm_write: String,
        #[arg(long, default_value = "Operator-confirmed recovery plan required")]
        recovery_plan: String,
        #[arg(long, default_value = "false")]
        dry_run: bool,

        // Explicit operator-confirmed identity (required for write safety)
        #[arg(long, default_value = "Gigabyte")]
        board_manufacturer: String,
        #[arg(long)]
        board_model: String,
        #[arg(long, default_value = "1.1")]
        board_rev: String,
        #[arg(long, default_value = "Macronix")]
        chip_vendor: String,
        #[arg(long)]
        chip_part: String,
        #[arg(long, default_value_t = 32 * 1024 * 1024)]
        chip_size: u64,
        /// Mark identities as OperatorConfirmed (must be true for write)
        #[arg(long, default_value = "true")]
        operator_confirmed: bool,
    },

    /// Safety-gated chip erase (same state machine as write)
    Erase {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        confirm_erase: String,
        #[arg(long, default_value = "Operator-confirmed recovery plan required")]
        recovery_plan: String,
        #[arg(long, default_value = "false")]
        dry_run: bool,
        #[arg(long, default_value = "Gigabyte")]
        board_manufacturer: String,
        #[arg(long)]
        board_model: String,
        #[arg(long, default_value = "1.1")]
        board_rev: String,
        #[arg(long, default_value = "Macronix")]
        chip_vendor: String,
        #[arg(long)]
        chip_part: String,
        #[arg(long, default_value_t = 32 * 1024 * 1024)]
        chip_size: u64,
        #[arg(long, default_value = "true")]
        operator_confirmed: bool,
    },

    /// Show audit report for an operation id
    Report {
        operation_id: String,
    },

    /// Same dependency detection as doctor; never installs or flashes
    SetupCheck,
}

#[derive(Subcommand, Debug)]
pub enum DeviceCmd {
    /// Read-only probe
    Probe {
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Doctor { programmer } => {
                let report = setup::run_doctor(&programmer)?;
                println!("=== bios-tool doctor ===");
                for item in &report.items {
                    println!(
                        "[{:?}] {}: {}",
                        item.status, item.name, item.detail
                    );
                }
                println!("\nProgrammer note: {}", report.programmer_note);
                Ok(())
            }
            Commands::Device(DeviceCmd::Probe { dry_run }) => {
                if dry_run {
                    println!("dry-run: would probe flashrom programmer");
                    return Ok(());
                }
                let backend = FlashromBackend::from_env_or_default();
                match backend.probe() {
                    Ok((chip, result)) => {
                        println!("=== device probe (read-only) ===");
                        println!("exit: {}", result.exit_code);
                        println!("chip vendor: {}", chip.vendor);
                        println!("part: {}", chip.part_number);
                        println!("capacity: {} bytes ({:.1} MiB)", chip.capacity_bytes, chip.size_mib());
                        println!("confidence: {:?}", chip.confidence);
                        println!("note: exact board SKU is NOT inferred from chipset/chip alone");
                        if let Some(raw) = &chip.probe_raw {
                            println!("--- probe excerpt ---\n{}", raw.chars().take(500).collect::<String>());
                        }
                    }
                    Err(e) => {
                        eprintln!("probe failed (this is expected if flashrom/programmer is unavailable on this host): {:#}", e);
                        println!("Detected facts: none (backend error)");
                        println!("Operator-supplied assumptions must be recorded separately and confirmed.");
                    }
                }
                Ok(())
            }
            Commands::Dump {
                output,
                reads,
                force,
                dry_run,
                board_model,
                board_rev,
            } => {
                if dry_run {
                    println!("dry-run: would dump {} times to {:?}", reads, output);
                    return Ok(());
                }
                let backend = FlashromBackend::from_env_or_default();
                let meta = acquisition::dump_full_chip(
                    &backend,
                    &output,
                    reads,
                    force,
                    &board_model,
                    &board_rev,
                )?;
                let meta_path = output.with_extension("json");
                acquisition::write_metadata_json(&meta, &meta_path)?;
                println!("dump saved: {:?}", output);
                println!("metadata: {:?}", meta_path);
                println!("sha256: {}", meta.final_sha256);
                Ok(())
            }
            Commands::Inspect { image } => {
                let report = firmware::load_and_inspect(&image)?;
                println!("=== inspect {:?} ===", image);
                println!("size: {} bytes", report.size);
                println!("sha256: {}", report.sha256);
                println!("probable kind: {:?}", report.probable_kind);
                println!("signatures: {:?}", report.signatures_found);
                println!("validated structures: {:?}", report.structures_validated);
                for n in &report.notes {
                    println!("note: {}", n);
                }
                for e in &report.errors {
                    println!("error: {}", e);
                }
                Ok(())
            }
            Commands::Compare { a, b } => {
                let ma = crate::domain::ImageMeta::from_path(&a)?;
                let mb = crate::domain::ImageMeta::from_path(&b)?;
                println!("=== compare ===");
                println!("A: {}  sha256={}", ma.size, ma.sha256);
                println!("B: {}  sha256={}", mb.size, mb.sha256);
                if ma.sha256 == mb.sha256 {
                    println!("identity: IDENTICAL");
                } else {
                    println!("identity: DIFFERENT");
                    println!("changed ranges: (full binary diff not expanded in this release; hashes differ)");
                }
                Ok(())
            }
            Commands::Validate { image, profile } => {
                let meta = crate::domain::ImageMeta::from_path(&image)?;
                let report = firmware::load_and_inspect(&image)?;
                let prof = validation::load_profile(&profile)?;
                match validation::validate_image_against_profile(&meta, &report, &prof) {
                    Ok(()) => {
                        println!("validation: PASS for profile {}", profile);
                        println!("note: board identity confidence is still {:?}", prof.identity.confidence);
                    }
                    Err(e) => {
                        println!("validation: FAIL — {:#}", e);
                        return Err(e);
                    }
                }
                Ok(())
            }
            Commands::Verify { image, dry_run } => {
                if dry_run {
                    println!("dry-run: would verify image against chip");
                    return Ok(());
                }
                let backend = FlashromBackend::from_env_or_default();
                match backend.verify_chip(&image) {
                    Ok(r) => {
                        println!("verify exit={}", r.exit_code);
                        if r.exit_code == 0 {
                            println!("chip matches image");
                        } else {
                            println!("mismatch or error: {}", r.stderr);
                        }
                    }
                    Err(e) => {
                        eprintln!("verify failed: {:#}", e);
                        return Err(e);
                    }
                }
                Ok(())
            }
            Commands::Write {
                image,
                backup,
                confirm_write,
                recovery_plan,
                dry_run,
                board_manufacturer,
                board_model,
                board_rev,
                chip_vendor,
                chip_part,
                chip_size,
                operator_confirmed,
            } => {
                if board_model.trim().is_empty() || chip_part.trim().is_empty() {
                    return Err(BiosToolError::BoardUncertain(
                        "board_model and chip_part are required for write".into(),
                    ));
                }
                if board_model.contains("provisional") || chip_part.contains("provisional") {
                    return Err(BiosToolError::BoardUncertain(
                        "provisional identities are never write-safe".into(),
                    ));
                }

                let conf = if operator_confirmed {
                    IdentityConfidence::OperatorConfirmed
                } else {
                    IdentityConfidence::Assumed
                };
                let chip_conf = if operator_confirmed {
                    ChipConfidence::OperatorSupplied
                } else {
                    ChipConfidence::Assumed
                };

                let board = BoardIdentity {
                    manufacturer: board_manufacturer,
                    model: board_model,
                    pcb_revision: board_rev,
                    confidence: conf,
                    notes: None,
                };
                let chip = ChipIdentity {
                    vendor: chip_vendor,
                    part_number: chip_part,
                    capacity_bytes: chip_size,
                    voltage_class: Some("3V".into()),
                    confidence: chip_conf,
                    probe_raw: None,
                };

                println!("=== WRITE SAFETY PREVIEW ===");
                println!("image: {:?}", image);
                if let Ok(m) = crate::domain::ImageMeta::from_path(&image) {
                    println!("image sha256: {}", m.sha256);
                    println!("image size: {}", m.size);
                }
                println!("backup: {:?}", backup);
                println!("board: {}  (confidence {:?})", board.exact_identity_string(), board.confidence);
                println!("chip: {} ({} bytes)  (confidence {:?})", chip.part_number, chip.capacity_bytes, chip.confidence);
                println!("confirm phrase required: exactly \"{}\"", board.exact_identity_string());
                println!("recovery: {}", recovery_plan);

                if dry_run {
                    println!("dry-run: write path not executed");
                    return Ok(());
                }

                let backend = FlashromBackend::from_env_or_default();
                let coord = WriteCoordinator::new(backend);
                match coord.execute_write(
                    &image,
                    &backup,
                    &confirm_write,
                    board,
                    chip,
                    &recovery_plan,
                    false,
                ) {
                    Ok(ctx) => {
                        println!("write completed; final state: {:?}", ctx.state);
                        let mut rec = AuditRecord::new("WriteSuccess");
                        rec.final_status = format!("{:?}", ctx.state);
                        let _ = rec.save(std::path::Path::new("logs"));
                    }
                    Err(e) => {
                        println!("write BLOCKED or failed: {:#}", e);
                        let mut rec = AuditRecord::new("WriteBlocked");
                        rec.final_status = format!("{:#}", e);
                        let _ = rec.save(std::path::Path::new("logs"));
                        return Err(e);
                    }
                }
                Ok(())
            }
            Commands::Erase {
                backup,
                confirm_erase,
                recovery_plan,
                dry_run,
                board_manufacturer,
                board_model,
                board_rev,
                chip_vendor,
                chip_part,
                chip_size,
                operator_confirmed,
            } => {
                if board_model.trim().is_empty() || chip_part.trim().is_empty() {
                    return Err(BiosToolError::BoardUncertain(
                        "board_model and chip_part are required for erase".into(),
                    ));
                }

                let conf = if operator_confirmed {
                    IdentityConfidence::OperatorConfirmed
                } else {
                    IdentityConfidence::Assumed
                };
                let chip_conf = if operator_confirmed {
                    ChipConfidence::OperatorSupplied
                } else {
                    ChipConfidence::Assumed
                };

                let board = BoardIdentity {
                    manufacturer: board_manufacturer,
                    model: board_model,
                    pcb_revision: board_rev,
                    confidence: conf,
                    notes: None,
                };
                let chip = ChipIdentity {
                    vendor: chip_vendor,
                    part_number: chip_part,
                    capacity_bytes: chip_size,
                    voltage_class: Some("3V".into()),
                    confidence: chip_conf,
                    probe_raw: None,
                };

                println!("=== ERASE SAFETY PREVIEW ===");
                println!("backup: {:?}", backup);
                println!("board: {}", board.exact_identity_string());
                println!("chip: {} ({} bytes)", chip.part_number, chip.capacity_bytes);
                println!("confirm phrase required: exactly \"{}\"", board.exact_identity_string());
                println!("recovery: {}", recovery_plan);

                if dry_run {
                    println!("dry-run: erase path not executed");
                    return Ok(());
                }

                let backend = FlashromBackend::from_env_or_default();
                let coord = WriteCoordinator::new(backend);
                match coord.execute_erase(
                    &backup,
                    &confirm_erase,
                    board,
                    chip,
                    &recovery_plan,
                    false,
                ) {
                    Ok(ctx) => {
                        println!("erase completed; final state: {:?}", ctx.state);
                        let mut rec = AuditRecord::new("EraseSuccess");
                        rec.final_status = format!("{:?}", ctx.state);
                        let _ = rec.save(std::path::Path::new("logs"));
                    }
                    Err(e) => {
                        println!("erase BLOCKED or failed: {:#}", e);
                        let mut rec = AuditRecord::new("EraseBlocked");
                        rec.final_status = format!("{:#}", e);
                        let _ = rec.save(std::path::Path::new("logs"));
                        return Err(e);
                    }
                }
                Ok(())
            }
            Commands::Report { operation_id } => {
                let path = PathBuf::from("logs").join(format!("{}.json", operation_id));
                match AuditRecord::load(&path) {
                    Ok(rec) => {
                        println!("{}", serde_json::to_string_pretty(&rec)?);
                    }
                    Err(e) => {
                        eprintln!("report not found or unreadable: {:#}", e);
                        return Err(e);
                    }
                }
                Ok(())
            }
            Commands::SetupCheck => {
                let report = setup::run_doctor("internal")?;
                println!("=== setup-check (no install, no flash) ===");
                for item in &report.items {
                    println!("[{:?}] {}: {}", item.status, item.name, item.detail);
                }
                Ok(())
            }
        }
    }
}
