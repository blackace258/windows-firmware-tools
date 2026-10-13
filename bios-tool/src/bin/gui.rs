//! Professional GUI entry point for bios-tool (Windows).
//! Build with: cargo build --release --features gui --bin bios-tool-gui

use bios_tool::gui::BiosToolGui;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 560.0])
            .with_title("bios-tool \u2014 SPI BIOS Controlled Programming"),
        ..Default::default()
    };

    eframe::run_native(
        "bios-tool",
        options,
        Box::new(|cc| Ok(Box::new(BiosToolGui::new(cc)))),
    )
}
