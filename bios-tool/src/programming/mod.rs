//! Safety-gated write / erase and immediate read-back verification.
//! CLI and GUI must never call the write backend directly; only this coordinator.

use crate::backend::FlashromBackend;
use crate::domain::{
    SafetyContext, BoardIdentity, ChipIdentity, ImageMeta, BiosToolError, Result, WriteState,
};
use std::path::Path;

pub struct WriteCoordinator {
    pub backend: FlashromBackend,
}

impl WriteCoordinator {
    pub fn new(backend: FlashromBackend) -> Self {
        Self { backend }
    }

    /// Full gated write path. Returns final SafetyContext.
    pub fn execute_write(
        &self,
        image_path: &Path,
        backup_path: &Path,
        confirm_identity: &str,
        board: BoardIdentity,
        chip: ChipIdentity,
        recovery_plan: &str,
        dry_run: bool,
    ) -> Result<SafetyContext> {
        let mut ctx = SafetyContext::new(dry_run);

        // 1. Identify
        ctx.identify(board, chip)?;

        // 2. Backup verified
        let backup_meta = ImageMeta::from_path(backup_path)?;
        let expected = ctx.chip.as_ref().map(|c| c.capacity_bytes).unwrap_or(0);
        ctx.verify_backup(backup_meta, expected)?;

        // 3. Image validated
        let candidate = ImageMeta::from_path(image_path)?;
        ctx.validate_image(candidate, expected)?;

        // 4. Recovery
        ctx.confirm_recovery(recovery_plan.to_string())?;

        // 5. Backend readiness
        let _ = self.backend.version()?;
        ctx.backend_ok = true;

        // 6. Authorize with exact phrase
        ctx.authorize_write(confirm_identity)?;

        // 7. Begin write
        ctx.begin_writing()?;

        // Actual write
        let write_result = self.backend.write_chip(image_path)?;
        eprintln!("write exit={}", write_result.exit_code);

        // 8. Immediate read-back verification
        let verify_result = self.backend.verify_chip(image_path);
        let ok = match verify_result {
            Ok(r) => r.exit_code == 0,
            Err(_) => false,
        };
        ctx.complete_readback(ok)?;

        Ok(ctx)
    }

    /// Full gated erase path (same state machine; no candidate image).
    pub fn execute_erase(
        &self,
        backup_path: &Path,
        confirm_identity: &str,
        board: BoardIdentity,
        chip: ChipIdentity,
        recovery_plan: &str,
        dry_run: bool,
    ) -> Result<SafetyContext> {
        let mut ctx = SafetyContext::new(dry_run);

        ctx.identify(board, chip)?;

        let backup_meta = ImageMeta::from_path(backup_path)?;
        let expected = ctx.chip.as_ref().map(|c| c.capacity_bytes).unwrap_or(0);
        ctx.verify_backup(backup_meta, expected)?;

        let dummy = ImageMeta {
            path: "ERASE_NO_IMAGE".into(),
            size: expected,
            sha256: "erase".into(),
            kind: crate::domain::ImageKind::RawFullChip,
            notes: vec!["erase operation \u2014 no candidate image".into()],
        };
        ctx.validate_image(dummy, expected)?;
        ctx.confirm_recovery(recovery_plan.to_string())?;

        let _ = self.backend.version()?;
        ctx.backend_ok = true;
        ctx.authorize_write(confirm_identity)?;
        ctx.begin_writing()?;

        let erase_result = self.backend.erase_chip()?;
        eprintln!("erase exit={}", erase_result.exit_code);

        let ok = erase_result.exit_code == 0;
        ctx.complete_readback(ok)?;

        Ok(ctx)
    }

    /// Mock-friendly path for tests: never touches hardware when dry_run or mock flag.
    pub fn execute_write_mocked(
        &self,
        ctx: &mut SafetyContext,
        confirm_identity: &str,
    ) -> Result<()> {
        ctx.authorize_write(confirm_identity)?;
        ctx.begin_writing()?;
        ctx.complete_readback(true)?;
        Ok(())
    }
}
