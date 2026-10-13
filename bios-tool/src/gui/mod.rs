//! Professional Windows GUI for bios-tool.
//! Dark modern theme, sidebar navigation, safety state visualization,
//! file pickers, and gated write/erase workflows.

use eframe::egui::{self, Color32, RichText};
use crate::domain::{
    BoardIdentity, ChipIdentity, IdentityConfidence, ChipConfidence,
    SafetyContext, WriteState,
};
use crate::backend::FlashromBackend;
use crate::programming::WriteCoordinator;
use crate::firmware;
use crate::acquisition;
use crate::setup;
use crate::audit::AuditRecord;
use std::path::PathBuf;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Dashboard,
    Probe,
    Dump,
    Inspect,
    Validate,
    Write,
    Erase,
    Logs,
}

pub struct BiosToolGui {
    tab: Tab,
    // Shared fields
    status_log: Vec<String>,
    last_error: Option<String>,

    // Probe
    probe_result: Option<String>,
    chip: ChipIdentity,
    board: BoardIdentity,

    // Dump
    dump_output: String,
    dump_reads: u32,
    dump_busy: bool,

    // Inspect / Validate
    image_path: String,
    inspect_report: Option<String>,
    profile_id: String,

    // Write / Erase
    write_image: String,
    write_backup: String,
    confirm_phrase: String,
    recovery_plan: String,
    dry_run: bool,
    safety_ctx: Option<SafetyContext>,
    write_busy: bool,

    // Backend
    programmer: String,
}

impl Default for BiosToolGui {
    fn default() -> Self {
        Self {
            tab: Tab::Dashboard,
            status_log: vec!["bios-tool GUI ready. All writes are safety-gated.".into()],
            last_error: None,
            probe_result: None,
            chip: ChipIdentity::default(),
            board: BoardIdentity::default(),
            dump_output: "backup.bin".into(),
            dump_reads: 2,
            dump_busy: false,
            image_path: String::new(),
            inspect_report: None,
            profile_id: "default".into(),
            write_image: String::new(),
            write_backup: String::new(),
            confirm_phrase: String::new(),
            recovery_plan: "external programmer + known-good dump".into(),
            dry_run: true,
            safety_ctx: None,
            write_busy: false,
            programmer: "internal".into(),
        }
    }
}

impl BiosToolGui {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.status_log.push(msg.into());
        if self.status_log.len() > 200 {
            self.status_log.remove(0);
        }
    }

    fn set_error(&mut self, e: impl std::fmt::Display) {
        let s = format!("{:#}", e);
        self.last_error = Some(s.clone());
        self.log(format!("ERROR: {}", s));
    }

    fn clear_error(&mut self) {
        self.last_error = None;
    }

    fn apply_dark_theme(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = Color32::from_rgb(18, 18, 22);
        style.visuals.panel_fill = Color32::from_rgb(24, 24, 30);
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 32, 40);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 50);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 70);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 70, 90);
        style.visuals.selection.bg_fill = Color32::from_rgb(0, 120, 215);
        style.visuals.override_text_color = Some(Color32::from_rgb(230, 230, 235));
        ctx.set_style(style);
    }
}

impl eframe::App for BiosToolGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::apply_dark_theme(ctx);

        // Top menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("bios-tool").strong().size(18.0));
                ui.label(RichText::new("  SPI BIOS · Controlled Programming").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Doctor").clicked() {
                        match setup::run_doctor(&self.programmer) {
                            Ok(r) => {
                                self.log("Doctor finished");
                                for i in r.items {
                                    self.log(format!("[{:?}] {}: {}", i.status, i.name, i.detail));
                                }
                            }
                            Err(e) => self.set_error(e),
                        }
                    }
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                });
            });
        });

        // Sidebar
        egui::SidePanel::left("sidebar")
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("NAVIGATION").small().weak());
                ui.separator();

                let tabs = [
                    (Tab::Dashboard, "Dashboard"),
                    (Tab::Probe, "Device Probe"),
                    (Tab::Dump, "Dump"),
                    (Tab::Inspect, "Inspect"),
                    (Tab::Validate, "Validate"),
                    (Tab::Write, "Write"),
                    (Tab::Erase, "Erase"),
                    (Tab::Logs, "Audit Logs"),
                ];
                for (t, label) in tabs {
                    let selected = self.tab == t;
                    if ui
                        .selectable_label(selected, RichText::new(label).size(14.0))
                        .clicked()
                    {
                        self.tab = t;
                        self.clear_error();
                    }
                }

                ui.add_space(20.0);
                ui.separator();
                ui.label(RichText::new("SAFETY").small().weak());
                ui.label(
                    RichText::new("Writes require exact\nboard identity phrase\nand verified backup.")
                        .small()
                        .color(Color32::from_rgb(255, 180, 80)),
                );
            });

        // Bottom status
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.last_error {
                    ui.colored_label(Color32::from_rgb(255, 80, 80), format!("⚠ {}", err));
                } else {
                    ui.label(RichText::new("Ready").color(Color32::from_rgb(80, 200, 120)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.dry_run, "Dry-run (safe)");
                });
            });
        });

        // Central content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Dashboard => self.ui_dashboard(ui),
                Tab::Probe => self.ui_probe(ui),
                Tab::Dump => self.ui_dump(ui),
                Tab::Inspect => self.ui_inspect(ui),
                Tab::Validate => self.ui_validate(ui),
                Tab::Write => self.ui_write(ui),
                Tab::Erase => self.ui_erase(ui),
                Tab::Logs => self.ui_logs(ui),
            }
        });
    }
}

impl BiosToolGui {
    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading("Dashboard");
        ui.add_space(8.0);
        ui.label("Professional Windows SPI BIOS tool with mandatory safety state machine.");
        ui.add_space(12.0);

        egui::Grid::new("dash_grid")
            .num_columns(2)
            .spacing([20.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Board").strong());
                ui.label(self.board.exact_identity_string());
                ui.end_row();
                ui.label(RichText::new("Confidence").strong());
                ui.label(format!("{:?}", self.board.confidence));
                ui.end_row();
                ui.label(RichText::new("Chip").strong());
                ui.label(format!("{} {}", self.chip.vendor, self.chip.part_number));
                ui.end_row();
                ui.label(RichText::new("Capacity").strong());
                ui.label(format!("{:.1} MiB", self.chip.size_mib()));
                ui.end_row();
                ui.label(RichText::new("Programmer").strong());
                ui.label(&self.programmer);
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.group(|ui| {
            ui.label(RichText::new("Safety state machine").strong());
            ui.label(
                "Unidentified → Identified → BackupVerified → ImageValidated →\n\
                 RecoveryConfirmed → WriteAuthorized → Writing → ReadbackVerified",
            );
            ui.label(RichText::new("Any failure → Blocked. No --yes bypass.").color(Color32::from_rgb(255, 160, 60)));
        });

        ui.add_space(12.0);
        if ui.button("Run environment doctor").clicked() {
            match setup::run_doctor(&self.programmer) {
                Ok(r) => {
                    self.log("Doctor OK");
                    for i in r.items {
                        self.log(format!("[{:?}] {}: {}", i.status, i.name, i.detail));
                    }
                    self.tab = Tab::Logs;
                }
                Err(e) => self.set_error(e),
            }
        }
    }

    fn ui_probe(&mut self, ui: &mut egui::Ui) {
        ui.heading("Device Probe (read-only)");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Programmer:");
            ui.text_edit_singleline(&mut self.programmer);
        });
        if ui.button("Probe flash chip").clicked() {
            let backend = FlashromBackend::from_env_or_default();
            match backend.probe() {
                Ok((chip, result)) => {
                    self.chip = chip.clone();
                    self.probe_result = Some(format!(
                        "exit {}\nvendor: {}\npart: {}\ncapacity: {} bytes\nconfidence: {:?}\n\n{}",
                        result.exit_code,
                        chip.vendor,
                        chip.part_number,
                        chip.capacity_bytes,
                        chip.confidence,
                        chip.probe_raw.as_deref().unwrap_or("")
                    ));
                    self.log(format!("Probe OK: {} {}", chip.vendor, chip.part_number));
                    self.clear_error();
                }
                Err(e) => {
                    self.probe_result = Some(format!("Probe failed: {:#}", e));
                    self.set_error(e);
                }
            }
        }
        ui.add_space(8.0);
        if let Some(r) = &self.probe_result {
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                ui.monospace(r);
            });
        }
    }

    fn ui_dump(&mut self, ui: &mut egui::Ui) {
        ui.heading("Full-chip Dump");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Output path:");
            ui.text_edit_singleline(&mut self.dump_output);
            if ui.button("Browse…").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name("backup.bin")
                    .save_file()
                {
                    self.dump_output = p.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Reads:");
            ui.add(egui::DragValue::new(&mut self.dump_reads).range(1..=5));
        });
        if ui.button("Start dump").clicked() {
            if self.dry_run {
                self.log(format!("dry-run: would dump {} times to {}", self.dump_reads, self.dump_output));
                return;
            }
            let backend = FlashromBackend::from_env_or_default();
            let out = PathBuf::from(&self.dump_output);
            match acquisition::dump_full_chip(
                &backend,
                &out,
                self.dump_reads,
                false,
                &self.board.model,
                &self.board.pcb_revision,
            ) {
                Ok(meta) => {
                    self.log(format!("Dump saved: {}  sha256={}", out.display(), meta.final_sha256));
                    let meta_path = out.with_extension("json");
                    let _ = acquisition::write_metadata_json(&meta, &meta_path);
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
        }
    }

    fn ui_inspect(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspect Image");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Image:");
            ui.text_edit_singleline(&mut self.image_path);
            if ui.button("Browse…").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Firmware", &["bin", "rom", "fd", "cap"])
                    .pick_file()
                {
                    self.image_path = p.display().to_string();
                }
            }
        });
        if ui.button("Inspect").clicked() {
            let path = PathBuf::from(&self.image_path);
            match firmware::load_and_inspect(&path) {
                Ok(r) => {
                    self.inspect_report = Some(format!(
                        "size: {} bytes\nsha256: {}\nkind: {:?}\nsignatures: {:?}\nstructures: {:?}\nnotes: {:?}\nerrors: {:?}",
                        r.size, r.sha256, r.probable_kind, r.signatures_found, r.structures_validated, r.notes, r.errors
                    ));
                    self.log("Inspect complete");
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
        }
        if let Some(r) = &self.inspect_report {
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                ui.monospace(r);
            });
        }
    }

    fn ui_validate(&mut self, ui: &mut egui::Ui) {
        ui.heading("Validate against Profile");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Image:");
            ui.text_edit_singleline(&mut self.image_path);
            if ui.button("Browse…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    self.image_path = p.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Profile:");
            ui.text_edit_singleline(&mut self.profile_id);
        });
        if ui.button("Validate").clicked() {
            let path = PathBuf::from(&self.image_path);
            match (crate::domain::ImageMeta::from_path(&path), firmware::load_and_inspect(&path)) {
                (Ok(meta), Ok(report)) => {
                    match crate::validation::load_profile(&self.profile_id)
                        .and_then(|p| crate::validation::validate_image_against_profile(&meta, &report, &p))
                    {
                        Ok(()) => {
                            self.log(format!("Validation PASS for profile {}", self.profile_id));
                            self.clear_error();
                        }
                        Err(e) => self.set_error(e),
                    }
                }
                (Err(e), _) | (_, Err(e)) => self.set_error(e),
            }
        }
    }

    fn ui_write(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Controlled Write").color(Color32::from_rgb(255, 140, 60)));
        ui.label("All steps of the safety state machine must pass. Exact confirmation phrase required.");
        ui.add_space(8.0);

        // Identity
        ui.group(|ui| {
            ui.label(RichText::new("Operator-confirmed board & chip identity").strong());
            ui.horizontal(|ui| {
                ui.label("Manufacturer:");
                ui.text_edit_singleline(&mut self.board.manufacturer);
            });
            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.text_edit_singleline(&mut self.board.model);
            });
            ui.horizontal(|ui| {
                ui.label("PCB rev:");
                ui.text_edit_singleline(&mut self.board.pcb_revision);
            });
            ui.horizontal(|ui| {
                ui.label("Chip vendor:");
                ui.text_edit_singleline(&mut self.chip.vendor);
            });
            ui.horizontal(|ui| {
                ui.label("Chip part:");
                ui.text_edit_singleline(&mut self.chip.part_number);
            });
            ui.horizontal(|ui| {
                ui.label("Size (bytes):");
                ui.add(egui::DragValue::new(&mut self.chip.capacity_bytes).speed(1024.0 * 1024.0));
            });
            self.board.confidence = IdentityConfidence::OperatorConfirmed;
            self.chip.confidence = ChipConfidence::OperatorSupplied;
            ui.label(format!("Required phrase: \"{}\"", self.board.exact_identity_string()));
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Candidate image:");
            ui.text_edit_singleline(&mut self.write_image);
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    self.write_image = p.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Same-board backup:");
            ui.text_edit_singleline(&mut self.write_backup);
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    self.write_backup = p.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Confirm phrase:");
            ui.text_edit_singleline(&mut self.confirm_phrase);
        });
        ui.horizontal(|ui| {
            ui.label("Recovery plan:");
            ui.text_edit_singleline(&mut self.recovery_plan);
        });

        // Visual state machine
        if let Some(ctx) = &self.safety_ctx {
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(RichText::new("Safety state").strong());
                let color = match &ctx.state {
                    WriteState::Blocked { .. } => Color32::from_rgb(220, 50, 50),
                    WriteState::ReadbackVerified => Color32::from_rgb(50, 200, 80),
                    WriteState::Writing | WriteState::WriteAuthorized => Color32::from_rgb(255, 180, 40),
                    _ => Color32::from_rgb(100, 160, 255),
                };
                ui.colored_label(color, format!("{:?}", ctx.state));
            });
        }

        ui.add_space(10.0);
        let write_label = if self.dry_run { "Dry-run write (no flash)" } else { "⚠ EXECUTE WRITE" };
        if ui.add(egui::Button::new(RichText::new(write_label).strong()).fill(Color32::from_rgb(180, 60, 40))).clicked() {
            self.board.confidence = IdentityConfidence::OperatorConfirmed;
            self.chip.confidence = ChipConfidence::OperatorSupplied;
            if self.board.model.contains("provisional") || self.chip.part_number.contains("provisional") {
                self.set_error("Provisional identities are never write-safe");
                return;
            }
            let backend = FlashromBackend::from_env_or_default();
            let coord = WriteCoordinator::new(backend);
            let img = PathBuf::from(&self.write_image);
            let bak = PathBuf::from(&self.write_backup);
            match coord.execute_write(
                &img,
                &bak,
                &self.confirm_phrase,
                self.board.clone(),
                self.chip.clone(),
                &self.recovery_plan,
                self.dry_run,
            ) {
                Ok(ctx) => {
                    self.safety_ctx = Some(ctx.clone());
                    if self.dry_run {
                        self.log(format!("Dry-run only (no flash). State: {:?}", ctx.state));
                    } else {
                        self.log(format!("Write finished: {:?}", ctx.state));
                    }
                    let mut rec = AuditRecord::new("WriteSuccess");
                    rec.final_status = format!("{:?}", ctx.state);
                    let _ = rec.save(std::path::Path::new("logs"));
                    self.clear_error();
                }
                Err(e) => {
                    self.set_error(&e);
                    let mut rec = AuditRecord::new("WriteBlocked");
                    rec.final_status = format!("{:#}", e);
                    let _ = rec.save(std::path::Path::new("logs"));
                }
            }
        }
    }

    fn ui_erase(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Controlled Erase").color(Color32::from_rgb(255, 80, 80)));
        ui.label("Identical safety gates as write. Backup must be verified first.");
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(RichText::new("Operator-confirmed identity").strong());
            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.text_edit_singleline(&mut self.board.model);
            });
            ui.horizontal(|ui| {
                ui.label("PCB rev:");
                ui.text_edit_singleline(&mut self.board.pcb_revision);
            });
            ui.horizontal(|ui| {
                ui.label("Chip part:");
                ui.text_edit_singleline(&mut self.chip.part_number);
            });
            self.board.confidence = IdentityConfidence::OperatorConfirmed;
            self.chip.confidence = ChipConfidence::OperatorSupplied;
            ui.label(format!("Required phrase: \"{}\"", self.board.exact_identity_string()));
        });

        ui.horizontal(|ui| {
            ui.label("Verified backup:");
            ui.text_edit_singleline(&mut self.write_backup);
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    self.write_backup = p.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Confirm phrase:");
            ui.text_edit_singleline(&mut self.confirm_phrase);
        });
        ui.horizontal(|ui| {
            ui.label("Recovery plan:");
            ui.text_edit_singleline(&mut self.recovery_plan);
        });

        ui.add_space(10.0);
        let label = if self.dry_run { "Dry-run erase" } else { "⚠ EXECUTE ERASE" };
        if ui.add(egui::Button::new(RichText::new(label).strong()).fill(Color32::from_rgb(160, 30, 30))).clicked() {
            let backend = FlashromBackend::from_env_or_default();
            let coord = WriteCoordinator::new(backend);
            let bak = PathBuf::from(&self.write_backup);
            match coord.execute_erase(
                &bak,
                &self.confirm_phrase,
                self.board.clone(),
                self.chip.clone(),
                &self.recovery_plan,
                self.dry_run,
            ) {
                Ok(ctx) => {
                    self.safety_ctx = Some(ctx.clone());
                    if self.dry_run {
                        self.log(format!("Dry-run erase only (no flash). State: {:?}", ctx.state));
                    } else {
                        self.log(format!("Erase finished: {:?}", ctx.state));
                    }
                    let mut rec = AuditRecord::new("EraseSuccess");
                    rec.final_status = format!("{:?}", ctx.state);
                    let _ = rec.save(std::path::Path::new("logs"));
                    self.clear_error();
                }
                Err(e) => {
                    self.set_error(&e);
                    let mut rec = AuditRecord::new("EraseBlocked");
                    rec.final_status = format!("{:#}", e);
                    let _ = rec.save(std::path::Path::new("logs"));
                }
            }
        }
    }

    fn ui_logs(&mut self, ui: &mut egui::Ui) {
        ui.heading("Status & Audit Log");
        ui.add_space(6.0);
        if ui.button("Clear").clicked() {
            self.status_log.clear();
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.status_log {
                    ui.monospace(line);
                }
            });
    }
}
