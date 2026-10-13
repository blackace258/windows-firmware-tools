//! Integration-style tests for the write safety state machine (no hardware).

use bios_tool::domain::{
    SafetyContext, WriteState, BoardIdentity, ChipIdentity,
    IdentityConfidence, ChipConfidence, ImageMeta, ImageKind, BiosToolError,
};

#[test]
fn starts_unidentified() {
    let ctx = SafetyContext::new(false);
    assert!(matches!(ctx.state, WriteState::Unidentified));
}

#[test]
fn blocks_provisional_board() {
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
    assert!(ctx.identify(board, chip).is_err());
    assert!(matches!(ctx.state, WriteState::Blocked { .. }));
}

#[test]
fn happy_path_to_authorized_then_dry_run() {
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
        path: "b.bin".into(),
        size: 32 * 1024 * 1024,
        sha256: "a".into(),
        kind: ImageKind::RawFullChip,
        notes: vec![],
    };
    ctx.verify_backup(backup, 32 * 1024 * 1024).unwrap();
    let image = ImageMeta {
        path: "i.bin".into(),
        size: 32 * 1024 * 1024,
        sha256: "b".into(),
        kind: ImageKind::RawFullChip,
        notes: vec![],
    };
    ctx.validate_image(image, 32 * 1024 * 1024).unwrap();
    ctx.confirm_recovery("plan".into()).unwrap();
    ctx.backend_ok = true;
    ctx.authorize_write(&board.exact_identity_string()).unwrap();
    assert!(matches!(ctx.begin_writing(), Err(BiosToolError::DryRun)));
}
