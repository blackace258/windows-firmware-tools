use smbios_tool::parser::parse_raw_table;

#[test]
fn empty_errors() {
    assert!(parse_raw_table(&[]).is_err());
}

#[test]
fn truncated_header_no_panic() {
    let _ = parse_raw_table(&[0, 1, 2]);
}

#[test]
fn oversized_length_no_panic() {
    let mut buf = vec![0u8; 8]; // RSMB-like
    buf.extend_from_slice(&[1u8, 200, 0, 0]); // type 1, len 200
    let r = parse_raw_table(&buf);
    assert!(r.is_ok());
}

#[test]
fn end_of_table_stops() {
    // Minimal: RSMB hdr + type 127
    let mut buf = vec![0u8; 8];
    buf.extend_from_slice(&[127, 4, 0, 0, 0, 0]); // type 127, len 4, handle 0, double nul
    let r = parse_raw_table(&buf).unwrap();
    assert!(r.structures.iter().any(|s| s.header.type_ == 127) || r.notes.iter().any(|n| n.contains("end-of-table")));
}
