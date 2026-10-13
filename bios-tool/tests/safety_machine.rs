use bios_tool::domain::safety::{SafetyContext, WriteState};
use bios_tool::domain::{BoardIdentity, ChipIdentity, ImageMeta, ImageKind, IdentityConfidence, ChipConfidence};

// Re-export path assumes lib; for binary crate we test via public modules or integrate as unit tests.
// These are conceptual integration-style tests; adjust if compiled as bin-only.

#[test]
fn state_starts_unidentified() {
    let ctx = SafetyContext::new(false);
    assert!(matches!(ctx.state, WriteState::Unidentified));
}

#[test]
fn blocks_on_provisional_board() {
    let mut ctx = SafetyContext::new(false);
    let board = BoardIdentity {
        manufacturer: "Gigabyte".into(),
        model: "B760M AORUS (provisional)".into(),
        pcb_revision: "1.1".into(),
        confidence: IdentityConfidence::Assumed,
        notes: None,
    };
    let chip = ChipIdentity {
        vendor: "Macronix".into(),
        part_number: "MX25L25673G".into(),
        capacity_bytes: 32 * 1024 * 1024,
        voltage_class: Some("3V".into()),
        confidence: ChipConfidence::OperatorSupplied,
        probe_raw: None,
    };
    let r = ctx.identify(board, chip);
    assert!(r.is_err());
    assert!(matches!(ctx.state, WriteState::Blocked { .. }));
}

#[test]
fn full_happy_path_dry_run() {
    let mut ctx = SafetyContext::new(true);
    let board = BoardIdentity {
        manufacturer: "Gigabyte".into(),
        model: "B760M AORUS".into(),
        pcb_revision: "1.1".into(),
        confidence: IdentityConfidence::OperatorConfirmed,
        notes: None,
    };
    let chip = ChipIdentity {
        vendor: "Macronix".into(),
        part_number: "MX25L25673G".into(),
        capacity_bytes: 32 * 1024 * 1024,
        voltage_class: Some("3V".into()),
        confidence: ChipConfidence::Probed,
        probe_raw: None,
    };
    ctx.identify(board.clone(), chip).unwrap();
    let backup = ImageMeta {
        path: "backup.bin".into(),
        size: 32 * 1024 * 1024,
        sha256: "abc".into(),
        kind: ImageKind::RawFullChip,
        notes: vec![],
    };
    ctx.verify_backup(backup, 32 * 1024 * 1024).unwrap();
    let image = ImageMeta {
        path: "image.bin".into(),
        size: 32 * 1024 * 1024,
        sha256: "def".into(),
        kind: ImageKind::RawFullChip,
        notes: vec![],
    };
    ctx.validate_image(image, 32 * 1024 * 1024).unwrap();
    ctx.confirm_recovery("external programmer + known good".into()).unwrap();
    ctx.backend_ok = true;
    ctx.authorize_write(&board.exact_identity_string()).unwrap();
    let r = ctx.begin_writing();
    assert!(matches!(r, Err(bios_tool::domain::BiosToolError::DryRun)));
}
