//! Human and machine-readable reports. Always distinguish published values
//! from any claim about persistent firmware storage.

use crate::acquisition::RawSmbiosTable;
use crate::parser::{ParseReport, DecodedFields};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReport {
    pub tool_version: String,
    pub acquisition: RawSmbiosTableMeta,
    pub parse: ParseSummary,
    pub type0_bios: Option<DecodedFields>,
    pub type1_system: Option<DecodedFields>,
    pub type2_baseboard: Option<DecodedFields>,
    pub type3_enclosure: Option<DecodedFields>,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSmbiosTableMeta {
    pub source: String,
    pub length: usize,
    pub sha256: String,
    pub acquired_at: String,
    pub provenance_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseSummary {
    pub structure_count: usize,
    pub smbios_version: Option<String>,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
}

pub fn build_json_report(raw: &RawSmbiosTable, parse: &ParseReport) -> JsonReport {
    let mut type0 = None;
    let mut type1 = None;
    let mut type2 = None;
    let mut type3 = None;
    for s in &parse.structures {
        match &s.decoded {
            Some(d @ DecodedFields::BiosInfo { .. }) if type0.is_none() => type0 = Some(d.clone()),
            Some(d @ DecodedFields::SystemInfo { .. }) if type1.is_none() => type1 = Some(d.clone()),
            Some(d @ DecodedFields::BaseboardInfo { .. }) if type2.is_none() => type2 = Some(d.clone()),
            Some(d @ DecodedFields::EnclosureInfo { .. }) if type3.is_none() => type3 = Some(d.clone()),
            _ => {}
        }
    }

    let ver = parse.entry_point.as_ref().map(|e| format!("{}.{}", e.major, e.minor));

    JsonReport {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        acquisition: RawSmbiosTableMeta {
            source: raw.source.clone(),
            length: raw.length,
            sha256: raw.sha256.clone(),
            acquired_at: raw.acquired_at.clone(),
            provenance_note: raw.provenance_note.clone(),
        },
        parse: ParseSummary {
            structure_count: parse.structure_count,
            smbios_version: ver,
            errors: parse.errors.clone(),
            notes: parse.notes.clone(),
        },
        type0_bios: type0,
        type1_system: type1,
        type2_baseboard: type2,
        type3_enclosure: type3,
        disclaimer: "All values are from the published SMBIOS table only. This tool does not read or write persistent firmware storage, UEFI variables, or SPI flash. A value that changes in a local buffer or even in a live table may still revert after reboot if the firmware source of truth was not updated through a supported platform path.".into(),
    }
}

pub fn print_human(report: &JsonReport) {
    println!("=== smbios-tool report ===");
    println!("source:      {}", report.acquisition.source);
    println!("length:      {}", report.acquisition.length);
    println!("sha256:      {}", report.acquisition.sha256);
    println!("acquired:    {}", report.acquisition.acquired_at);
    if let Some(ref v) = report.parse.smbios_version {
        println!("SMBIOS ver:  {}", v);
    }
    println!("structures:  {}", report.parse.structure_count);
    for n in &report.parse.notes {
        println!("note:        {}", n);
    }
    for e in &report.parse.errors {
        println!("parse-error: {}", e);
    }
    println!();
    if let Some(DecodedFields::BiosInfo { vendor, version, release_date }) = &report.type0_bios {
        println!("[Type 0 BIOS]");
        println!("  vendor:       {}", vendor.as_deref().unwrap_or("(none)"));
        println!("  version:      {}", version.as_deref().unwrap_or("(none)"));
        println!("  release_date: {}", release_date.as_deref().unwrap_or("(none)"));
    }
    if let Some(DecodedFields::SystemInfo {
        manufacturer, product_name, version, serial_number, uuid, sku, family,
    }) = &report.type1_system
    {
        println!("[Type 1 System]");
        println!("  manufacturer: {}", manufacturer.as_deref().unwrap_or("(none)"));
        println!("  product:      {}", product_name.as_deref().unwrap_or("(none)"));
        println!("  version:      {}", version.as_deref().unwrap_or("(none)"));
        println!("  serial:       {}", serial_number.as_deref().unwrap_or("(none)"));
        println!("  uuid:         {}", uuid.as_deref().unwrap_or("(none)"));
        println!("  sku:          {}", sku.as_deref().unwrap_or("(none)"));
        println!("  family:       {}", family.as_deref().unwrap_or("(none)"));
    }
    if let Some(DecodedFields::BaseboardInfo {
        manufacturer, product, version, serial_number, asset_tag,
    }) = &report.type2_baseboard
    {
        println!("[Type 2 Baseboard]");
        println!("  manufacturer: {}", manufacturer.as_deref().unwrap_or("(none)"));
        println!("  product:      {}", product.as_deref().unwrap_or("(none)"));
        println!("  version:      {}", version.as_deref().unwrap_or("(none)"));
        println!("  serial:       {}", serial_number.as_deref().unwrap_or("(none)"));
        println!("  asset_tag:    {}", asset_tag.as_deref().unwrap_or("(none)"));
    }
    if let Some(DecodedFields::EnclosureInfo {
        manufacturer, chassis_type, version, serial_number, asset_tag,
    }) = &report.type3_enclosure
    {
        println!("[Type 3 Enclosure]");
        println!("  manufacturer: {}", manufacturer.as_deref().unwrap_or("(none)"));
        println!("  chassis_type: {:?}", chassis_type);
        println!("  version:      {}", version.as_deref().unwrap_or("(none)"));
        println!("  serial:       {}", serial_number.as_deref().unwrap_or("(none)"));
        println!("  asset_tag:    {}", asset_tag.as_deref().unwrap_or("(none)"));
    }
    println!();
    println!("DISCLAIMER: {}", report.disclaimer);
}
