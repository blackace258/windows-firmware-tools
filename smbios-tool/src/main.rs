//! smbios-tool: Windows-native SMBIOS/DMI acquisition, parsing, comparison, reporting,
//! and gated write workflow (external vendor tool only; fail-closed).
//!
//! Core principle: reading a published SMBIOS table, changing a local copy, and
//! changing the firmware-managed source are distinct operations.

use clap::{Parser, Subcommand};
use smbios_tool::acquisition::{self};
use smbios_tool::parser;
use smbios_tool::report;
use smbios_tool::compare;
use smbios_tool::capability;
use smbios_tool::write::{
    self, BoardContext, FieldUpdate, WriteRequest, CONFIRM_PHRASE_PREFIX,
};
use smbios_tool::Result;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "smbios-tool")]
#[command(
    about = "SMBIOS/DMI table tool for Windows — read always; write only via gated vendor-tool path",
    long_about = None
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Acquire the published SMBIOS table (Windows RSMB) and optionally save raw + JSON
    Read {
        #[arg(long)]
        raw_out: Option<PathBuf>,
        #[arg(long)]
        json_out: Option<PathBuf>,
        #[arg(long)]
        from_file: Option<PathBuf>,
    },

    /// Parse and display a raw table (from live or file)
    Inspect {
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long)]
        json_out: Option<PathBuf>,
    },

    /// Compare two raw table captures
    Compare {
        a: PathBuf,
        b: PathBuf,
        #[arg(long)]
        json_out: Option<PathBuf>,
    },

    /// Emit platform capability report for the target board family
    Capability {
        #[arg(long, default_value = "gigabyte-b760m-aorus-rev1.1")]
        profile: String,
        #[arg(long)]
        json_out: Option<PathBuf>,
    },

    /// Gated DMI write via operator-supplied vendor utility (fail-closed).
    /// Does not patch SPI images or the published table in memory.
    Write {
        /// Board model string (must match physical label)
        #[arg(long)]
        board_model: String,
        /// PCB revision (must match physical label)
        #[arg(long)]
        board_rev: String,
        /// Exact identity used in confirmation phrase (default: "<model> PCB:<rev>")
        #[arg(long)]
        exact_identity: Option<String>,
        /// Field to update, e.g. type1.serial_number (repeatable)
        #[arg(long = "field", required = true)]
        fields: Vec<String>,
        /// New value for each --field (same order, same count)
        #[arg(long = "value", required = true)]
        values: Vec<String>,
        /// Path to previously saved raw SMBIOS backup from this board
        #[arg(long)]
        backup: PathBuf,
        /// Concrete recovery plan text
        #[arg(long)]
        recovery_plan: String,
        /// Exact phrase: I AUTHORIZE DMI WRITE ON <exact_identity>
        #[arg(long)]
        confirm_write: String,
        /// Path to operator-supplied vendor utility (required unless --dry-run)
        #[arg(long)]
        vendor_tool: Option<PathBuf>,
        /// Arguments for the vendor tool (required for non-dry-run)
        #[arg(long = "vendor-arg", num_args = 0..)]
        vendor_args: Vec<String>,
        /// Optional expected SHA-256 of the vendor tool binary
        #[arg(long)]
        vendor_sha256: Option<String>,
        /// Plan and validate only; never spawn vendor tool
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Write audit JSON to this path
        #[arg(long)]
        audit_out: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Read {
            raw_out,
            json_out,
            from_file,
        } => {
            let table = if let Some(p) = from_file {
                acquisition::load_from_file(&p)?
            } else {
                acquisition::read_published_table()?
            };
            let parse = parser::parse_raw_table(&table.raw_bytes)?;
            let report = report::build_json_report(&table, &parse);
            report::print_human(&report);

            if let Some(p) = raw_out {
                acquisition::save_raw(&table, &p)?;
                println!("raw saved: {:?}", p);
            }
            if let Some(p) = json_out {
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &report)?;
                println!("json saved: {:?}", p);
            }
            Ok(())
        }
        Commands::Inspect { from_file, json_out } => {
            let table = if let Some(p) = from_file {
                acquisition::load_from_file(&p)?
            } else {
                acquisition::read_published_table()?
            };
            let parse = parser::parse_raw_table(&table.raw_bytes)?;
            let report = report::build_json_report(&table, &parse);
            report::print_human(&report);
            if let Some(p) = json_out {
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &report)?;
                println!("json saved: {:?}", p);
            }
            Ok(())
        }
        Commands::Compare { a, b, json_out } => {
            let ta = acquisition::load_from_file(&a)?;
            let tb = acquisition::load_from_file(&b)?;
            let pa = parser::parse_raw_table(&ta.raw_bytes)?;
            let pb = parser::parse_raw_table(&tb.raw_bytes)?;
            let diff = compare::compare(&ta.sha256, &pa, &tb.sha256, &pb);
            println!("=== compare ===");
            println!("A sha256: {}", diff.a_sha256);
            println!("B sha256: {}", diff.b_sha256);
            println!("identical raw: {}", diff.identical_raw);
            for n in &diff.notes {
                println!("note: {}", n);
            }
            for d in &diff.field_diffs {
                println!(
                    "diff type{} {}: {:?} -> {:?}",
                    d.structure_type, d.field, d.value_a, d.value_b
                );
            }
            if let Some(p) = json_out {
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &diff)?;
                println!("json saved: {:?}", p);
            }
            Ok(())
        }
        Commands::Capability { profile, json_out } => {
            let r = if profile == "gigabyte-b760m-aorus-rev1.1"
                || profile.contains("b760")
                || profile == "default"
            {
                capability::gigabyte_b760m_aorus_rev11()
            } else {
                capability::unknown_profile(&profile)
            };
            capability::print_capability(&r);
            if let Some(p) = json_out {
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &r)?;
                println!("json saved: {:?}", p);
            }
            Ok(())
        }
        Commands::Write {
            board_model,
            board_rev,
            exact_identity,
            fields,
            values,
            backup,
            recovery_plan,
            confirm_write,
            vendor_tool,
            vendor_args,
            vendor_sha256,
            dry_run,
            audit_out,
        } => {
            if fields.len() != values.len() {
                return Err(smbios_tool::SmbiosError::Other(
                    "--field and --value counts must match".into(),
                ));
            }
            let identity = exact_identity
                .unwrap_or_else(|| format!("{} PCB:{}", board_model, board_rev));

            let updates: Vec<FieldUpdate> = fields
                .into_iter()
                .zip(values.into_iter())
                .map(|(field, new_value)| FieldUpdate { field, new_value })
                .collect();

            let req = WriteRequest {
                board: BoardContext {
                    model: board_model,
                    pcb_revision: board_rev,
                    exact_identity: identity.clone(),
                    operator_confirmed: true,
                },
                updates,
                backup_raw_path: backup,
                recovery_plan,
                confirm_phrase: confirm_write,
                dry_run,
                vendor_tool_path: vendor_tool,
                vendor_tool_args: vendor_args,
                vendor_tool_sha256: vendor_sha256,
            };

            println!("=== gated DMI write workflow ===");
            println!("exact identity: {}", identity);
            println!(
                "required phrase: {} {}",
                CONFIRM_PHRASE_PREFIX, identity
            );
            println!("dry_run: {}", dry_run);

            let audit = write::run_write_workflow(req)?;
            println!("operation_id: {}", audit.operation_id);
            println!("final state: {:?}", audit.state);
            for m in &audit.messages {
                println!("  • {}", m);
            }
            if let Some(p) = audit_out {
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &audit)?;
                println!("audit saved: {:?}", p);
            }
            println!();
            println!(
                "Persistence is only proven by an independent re-read after reboot:"
            );
            println!("  smbios-tool read --raw-out after.bin");
            println!("  smbios-tool compare <backup> after.bin");
            Ok(())
        }
    }
}
