//! Compare two published-table captures. Does not claim anything about
//! persistent firmware storage.

use crate::parser::{ParseReport, DecodedFields, ParsedStructure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    pub a_sha256: String,
    pub b_sha256: String,
    pub identical_raw: bool,
    pub structure_count_a: usize,
    pub structure_count_b: usize,
    pub field_diffs: Vec<FieldDiff>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    pub structure_type: u8,
    pub field: String,
    pub value_a: Option<String>,
    pub value_b: Option<String>,
}

pub fn compare(
    sha_a: &str,
    parse_a: &ParseReport,
    sha_b: &str,
    parse_b: &ParseReport,
) -> DiffReport {
    let mut diffs = Vec::new();
    let mut notes = Vec::new();

    if sha_a == sha_b {
        notes.push("raw tables are byte-identical".into());
    } else {
        notes.push("raw tables differ".into());
    }

    // Compare decoded Type 0–3 if present
    compare_decoded(
        find_decoded(parse_a, 0),
        find_decoded(parse_b, 0),
        0,
        &mut diffs,
    );
    compare_decoded(
        find_decoded(parse_a, 1),
        find_decoded(parse_b, 1),
        1,
        &mut diffs,
    );
    compare_decoded(
        find_decoded(parse_a, 2),
        find_decoded(parse_b, 2),
        2,
        &mut diffs,
    );
    compare_decoded(
        find_decoded(parse_a, 3),
        find_decoded(parse_b, 3),
        3,
        &mut diffs,
    );

    if parse_a.structure_count != parse_b.structure_count {
        notes.push(format!(
            "structure count differs: {} vs {}",
            parse_a.structure_count, parse_b.structure_count
        ));
    }

    DiffReport {
        a_sha256: sha_a.to_string(),
        b_sha256: sha_b.to_string(),
        identical_raw: sha_a == sha_b,
        structure_count_a: parse_a.structure_count,
        structure_count_b: parse_b.structure_count,
        field_diffs: diffs,
        notes,
    }
}

fn find_decoded(p: &ParseReport, type_: u8) -> Option<&DecodedFields> {
    p.structures
        .iter()
        .find(|s| s.header.type_ == type_)
        .and_then(|s| s.decoded.as_ref())
}

fn compare_decoded(
    a: Option<&DecodedFields>,
    b: Option<&DecodedFields>,
    type_: u8,
    out: &mut Vec<FieldDiff>,
) {
    match (a, b) {
        (None, None) => {}
        (Some(_), None) => out.push(FieldDiff {
            structure_type: type_,
            field: "(structure present)".into(),
            value_a: Some("present".into()),
            value_b: Some("absent".into()),
        }),
        (None, Some(_)) => out.push(FieldDiff {
            structure_type: type_,
            field: "(structure present)".into(),
            value_a: Some("absent".into()),
            value_b: Some("present".into()),
        }),
        (Some(da), Some(db)) => match (da, db) {
            (
                DecodedFields::SystemInfo {
                    manufacturer: ma,
                    product_name: pa,
                    serial_number: sa,
                    uuid: ua,
                    ..
                },
                DecodedFields::SystemInfo {
                    manufacturer: mb,
                    product_name: pb,
                    serial_number: sb,
                    uuid: ub,
                    ..
                },
            ) => {
                push_if_diff(out, type_, "manufacturer", ma, mb);
                push_if_diff(out, type_, "product_name", pa, pb);
                push_if_diff(out, type_, "serial_number", sa, sb);
                push_if_diff(out, type_, "uuid", ua, ub);
            }
            (
                DecodedFields::BaseboardInfo {
                    manufacturer: ma,
                    product: pa,
                    serial_number: sa,
                    ..
                },
                DecodedFields::BaseboardInfo {
                    manufacturer: mb,
                    product: pb,
                    serial_number: sb,
                    ..
                },
            ) => {
                push_if_diff(out, type_, "manufacturer", ma, mb);
                push_if_diff(out, type_, "product", pa, pb);
                push_if_diff(out, type_, "serial_number", sa, sb);
            }
            (
                DecodedFields::BiosInfo {
                    vendor: va,
                    version: vera,
                    ..
                },
                DecodedFields::BiosInfo {
                    vendor: vb,
                    version: verb,
                    ..
                },
            ) => {
                push_if_diff(out, type_, "vendor", va, vb);
                push_if_diff(out, type_, "version", vera, verb);
            }
            _ => {}
        },
    }
}

fn push_if_diff(
    out: &mut Vec<FieldDiff>,
    type_: u8,
    field: &str,
    a: &Option<String>,
    b: &Option<String>,
) {
    if a != b {
        out.push(FieldDiff {
            structure_type: type_,
            field: field.to_string(),
            value_a: a.clone(),
            value_b: b.clone(),
        });
    }
}
