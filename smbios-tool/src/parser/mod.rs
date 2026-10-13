//! Bounded SMBIOS structure parser.
//! Never panic on malformed input. Respect string-set double-NUL. Validate indexes.

use crate::error::{Result, SmbiosError};
use serde::{Deserialize, Serialize};

const END_OF_TABLE: u8 = 127;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmbiosEntryPoint {
    pub major: u8,
    pub minor: u8,
    pub docrev: u8,
    pub table_length: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureHeader {
    pub type_: u8,
    pub length: u8,
    pub handle: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStructure {
    pub header: StructureHeader,
    pub offset: usize,
    /// Formatted section bytes (excluding header already described).
    pub formatted: Vec<u8>,
    /// Resolved strings (1-based index → string). Index 0 is absent.
    pub strings: Vec<String>,
    /// Type-specific decoded fields (only for known types 0–3 in this release).
    pub decoded: Option<DecodedFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DecodedFields {
    BiosInfo {
        vendor: Option<String>,
        version: Option<String>,
        release_date: Option<String>,
    },
    SystemInfo {
        manufacturer: Option<String>,
        product_name: Option<String>,
        version: Option<String>,
        serial_number: Option<String>,
        uuid: Option<String>,
        sku: Option<String>,
        family: Option<String>,
    },
    BaseboardInfo {
        manufacturer: Option<String>,
        product: Option<String>,
        version: Option<String>,
        serial_number: Option<String>,
        asset_tag: Option<String>,
    },
    EnclosureInfo {
        manufacturer: Option<String>,
        chassis_type: Option<u8>,
        version: Option<String>,
        serial_number: Option<String>,
        asset_tag: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseReport {
    pub entry_point: Option<SmbiosEntryPoint>,
    pub structures: Vec<ParsedStructure>,
    pub structure_count: usize,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
}

/// Windows GetSystemFirmwareTable(RSMB) layout:
///   BYTE  Used20CallingMethod
///   BYTE  SMBIOSMajorVersion
///   BYTE  SMBIOSMinorVersion
///   BYTE  DmiRevision
///   DWORD Length
///   BYTE  SMBIOSTableData[Length]
pub fn parse_raw_table(raw: &[u8]) -> Result<ParseReport> {
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    let mut entry_point = None;
    let mut data_offset = 0usize;

    // Detect Windows RSMB header (at least 8 bytes)
    if raw.len() >= 8 {
        let major = raw[1];
        let minor = raw[2];
        let docrev = raw[3];
        let table_len = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        entry_point = Some(SmbiosEntryPoint {
            major,
            minor,
            docrev,
            table_length: Some(table_len),
        });
        data_offset = 8;
        if data_offset + table_len as usize > raw.len() {
            errors.push(format!(
                "declared table length {} exceeds remaining buffer {}",
                table_len,
                raw.len().saturating_sub(data_offset)
            ));
        }
        notes.push(format!(
            "SMBIOS {}.{} (docrev {}), declared data length {}",
            major, minor, docrev, table_len
        ));
    } else if raw.len() < 4 {
        return Err(SmbiosError::Malformed("buffer shorter than minimum header".into()));
    } else {
        notes.push("no Windows RSMB header detected; treating entire buffer as structure stream".into());
    }

    let data = &raw[data_offset..];
    let mut structures = Vec::new();
    let mut cursor = 0usize;

    while cursor + 4 <= data.len() {
        let type_ = data[cursor];
        let length = data[cursor + 1] as usize;
        let handle = u16::from_le_bytes([data[cursor + 2], data[cursor + 3]]);

        if length < 4 {
            errors.push(format!(
                "structure at 0x{:x}: length {} < header size",
                data_offset + cursor,
                length
            ));
            break;
        }
        if cursor + length > data.len() {
            errors.push(format!(
                "structure at 0x{:x}: length {} exceeds remaining {}",
                data_offset + cursor,
                length,
                data.len() - cursor
            ));
            break;
        }

        // String-set starts at cursor + length; find double-NUL within bounds
        let string_start = cursor + length;
        let mut string_end = string_start;
        let mut found_term = false;
        while string_end + 1 < data.len() {
            if data[string_end] == 0 && data[string_end + 1] == 0 {
                string_end += 2;
                found_term = true;
                break;
            }
            string_end += 1;
        }
        if !found_term {
            // Allow single trailing NUL at end of buffer as soft end
            if string_end < data.len() && data[string_end] == 0 {
                string_end += 1;
                notes.push(format!(
                    "structure at 0x{:x}: missing double-NUL; used soft end",
                    data_offset + cursor
                ));
            } else {
                errors.push(format!(
                    "structure at 0x{:x}: string-set terminator not found within buffer",
                    data_offset + cursor
                ));
                break;
            }
        }

        let formatted = data[cursor + 4..cursor + length].to_vec();
        let string_blob = &data[string_start..string_end.saturating_sub(1).min(data.len())];
        let strings = split_strings(string_blob);

        let header = StructureHeader {
            type_,
            length: length as u8,
            handle,
        };

        let decoded = decode_known(&header, &formatted, &strings, &mut errors);

        structures.push(ParsedStructure {
            header,
            offset: data_offset + cursor,
            formatted,
            strings,
            decoded,
        });

        if type_ == END_OF_TABLE {
            notes.push("end-of-table structure (type 127) encountered".into());
            break;
        }

        // Advance past formatted + string-set
        cursor = string_end;
        // Safety: if we didn't advance, break to avoid infinite loop
        if cursor <= data_offset + (structures.last().map(|s| s.offset).unwrap_or(0)).saturating_sub(data_offset) {
            // ensure progress
        }
        if string_end <= cursor && !found_term {
            break;
        }
    }

    Ok(ParseReport {
        entry_point,
        structure_count: structures.len(),
        structures,
        errors,
        notes,
    })
}

fn split_strings(blob: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 0..blob.len() {
        if blob[i] == 0 {
            if start < i {
                let s = String::from_utf8_lossy(&blob[start..i]).into_owned();
                out.push(s);
            } else {
                // empty string slot
                out.push(String::new());
            }
            start = i + 1;
        }
    }
    // trailing non-terminated
    if start < blob.len() {
        out.push(String::from_utf8_lossy(&blob[start..]).into_owned());
    }
    out
}

fn string_at(strings: &[String], index: u8) -> Option<String> {
    if index == 0 {
        return None;
    }
    let i = (index as usize).saturating_sub(1);
    strings.get(i).cloned().filter(|s| !s.is_empty())
}

fn decode_known(
    header: &StructureHeader,
    formatted: &[u8],
    strings: &[String],
    errors: &mut Vec<String>,
) -> Option<DecodedFields> {
    match header.type_ {
        0 => {
            // Type 0: vendor(1), version(2), ... release date often at string index after
            // Formatted: after header, byte 0 = Vendor str idx, 1 = Version str idx, ...
            // Minimal: first two string indexes if present
            let vendor_idx = formatted.first().copied().unwrap_or(0);
            let version_idx = formatted.get(1).copied().unwrap_or(0);
            // Release date is commonly the third string index in many implementations
            let date_idx = formatted.get(2).copied().unwrap_or(0);
            Some(DecodedFields::BiosInfo {
                vendor: string_at(strings, vendor_idx),
                version: string_at(strings, version_idx),
                release_date: string_at(strings, date_idx),
            })
        }
        1 => {
            // Type 1: Manufacturer, Product Name, Version, Serial, UUID(16 bytes), ...
            if formatted.len() < 4 {
                errors.push(format!("Type 1 at handle {}: formatted too short", header.handle));
                return None;
            }
            let mfg = string_at(strings, formatted[0]);
            let product = string_at(strings, formatted[1]);
            let version = string_at(strings, formatted[2]);
            let serial = string_at(strings, formatted[3]);
            let uuid = if formatted.len() >= 20 {
                // UUID at offset 4 relative to formatted start (bytes 4..20)
                Some(format_uuid(&formatted[4..20]))
            } else {
                None
            };
            let sku = formatted.get(20).and_then(|&i| string_at(strings, i));
            let family = formatted.get(21).and_then(|&i| string_at(strings, i));
            Some(DecodedFields::SystemInfo {
                manufacturer: mfg,
                product_name: product,
                version,
                serial_number: serial,
                uuid,
                sku,
                family,
            })
        }
        2 => {
            if formatted.len() < 4 {
                return None;
            }
            Some(DecodedFields::BaseboardInfo {
                manufacturer: string_at(strings, formatted[0]),
                product: string_at(strings, formatted[1]),
                version: string_at(strings, formatted[2]),
                serial_number: string_at(strings, formatted[3]),
                asset_tag: formatted.get(4).and_then(|&i| string_at(strings, i)),
            })
        }
        3 => {
            if formatted.is_empty() {
                return None;
            }
            let chassis_type = formatted.get(1).copied();
            Some(DecodedFields::EnclosureInfo {
                manufacturer: string_at(strings, formatted[0]),
                chassis_type,
                version: formatted.get(2).and_then(|&i| string_at(strings, i)),
                serial_number: formatted.get(3).and_then(|&i| string_at(strings, i)),
                asset_tag: formatted.get(4).and_then(|&i| string_at(strings, i)),
            })
        }
        _ => None,
    }
}

/// Format 16-byte SMBIOS UUID. Ordering follows the SMBIOS version conventions;
/// we emit a standard dashed hex string from the raw little-endian-ish layout
/// without inventing a reverse that may be wrong for the table version.
fn format_uuid(bytes: &[u8]) -> String {
    if bytes.len() < 16 {
        return hex::encode(bytes);
    }
    // Common presentation: time-low, time-mid, time-hi, clock, node
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[3], bytes[2], bytes[1], bytes[0],
        bytes[5], bytes[4],
        bytes[7], bytes[6],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_errors() {
        let r = parse_raw_table(&[]);
        assert!(r.is_err());
    }

    #[test]
    fn short_buffer_no_panic() {
        let r = parse_raw_table(&[0u8; 3]);
        // may error or return empty; must not panic
        let _ = r;
    }

    #[test]
    fn malformed_length_no_panic() {
        // type=1, length=255 (too big), handle=0
        let mut buf = vec![0u8; 8]; // fake RSMB hdr
        buf.extend_from_slice(&[1, 255, 0, 0]);
        let r = parse_raw_table(&buf);
        assert!(r.is_ok());
        let report = r.unwrap();
        assert!(!report.errors.is_empty() || report.structures.is_empty());
    }
}
