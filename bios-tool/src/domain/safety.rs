//! Mandatory write-safety state machine.
//! Unidentified -> Identified -> BackupVerified -> ImageValidated -> RecoveryConfirmed
//! -> WriteAuthorized -> Writing -> ReadbackVerified
//! Any failure -> Blocked. Only one coordinator may invoke the write backend.

use serde::{Deserialize, Serialize};
use crate::domain::{BoardIdentity, ChipIdentity, ImageMeta, BiosToolError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WriteState {
    Unidentified,
    Identified,
    BackupVerified,
    ImageValidated,
    RecoveryConfirmed,
    WriteAuthorized,
    Writing,
    ReadbackVerified,
    Blocked { reason: String },
}

impl Default for WriteState {
    fn default() -> Self {
        WriteState::Unidentified
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyContext {
    pub state: WriteState,
    pub board: Option<BoardIdentity>,
    pub chip: Option<ChipIdentity>,
    pub backup: Option<ImageMeta>,
    pub candidate: Option<ImageMeta>,
    pub recovery_plan: Option<String>,
    pub backend_ok: bool,
    pub dry_run: bool,
}

impl SafetyContext {
    pub fn new(dry_run: bool) -> Self {
        Self {
            state: WriteState::Unidentified,
            board: None,
            chip: None,
            backup: None,
            candidate: None,
            recovery_plan: None,
            backend_ok: false,
            dry_run,
        }
    }

    pub fn identify(&mut self, board: BoardIdentity, chip: ChipIdentity) -> Result<()> {
        if !board.is_write_safe() {
            self.state = WriteState::Blocked {
                reason: format!("board identity not operator-confirmed: {}", board.exact_identity_string()),
            };
            return Err(BiosToolError::BoardUncertain(board.exact_identity_string()));
        }
        if !chip.is_write_safe() {
            self.state = WriteState::Blocked {
                reason: format!("chip identity uncertain: {}", chip.part_number),
            };
            return Err(BiosToolError::ChipUncertain(chip.part_number.clone()));
        }
        self.board = Some(board);
        self.chip = Some(chip);
        self.state = WriteState::Identified;
        Ok(())
    }

    pub fn verify_backup(&mut self, backup: ImageMeta, expected_size: u64) -> Result<()> {
        self.require_state(&[WriteState::Identified])?;
        if backup.size != expected_size {
            self.block(format!("backup size mismatch: {} != {}", backup.size, expected_size));
            return Err(BiosToolError::BackupInvalid(format!(
                "size {} != expected {}",
                backup.size, expected_size
            )));
        }
        if backup.sha256.is_empty() {
            self.block("backup hash missing".into());
            return Err(BiosToolError::BackupInvalid("missing hash".into()));
        }
        self.backup = Some(backup);
        self.state = WriteState::BackupVerified;
        Ok(())
    }

    pub fn validate_image(&mut self, image: ImageMeta, expected_size: u64) -> Result<()> {
        self.require_state(&[WriteState::BackupVerified])?;
        if image.size != expected_size {
            self.block(format!("image size mismatch: {} != {}", image.size, expected_size));
            return Err(BiosToolError::ImageInvalid(format!(
                "size {} != expected {}",
                image.size, expected_size
            )));
        }
        if matches!(image.kind, crate::domain::ImageKind::Capsule) {
            self.block("capsule image cannot be written as full-chip raw".into());
            return Err(BiosToolError::ImageInvalid(
                "capsule confused with raw full-chip".into(),
            ));
        }
        if matches!(image.kind, crate::domain::ImageKind::Unknown) {
            self.block("image kind unknown / ambiguous".into());
            return Err(BiosToolError::ImageInvalid("ambiguous image type".into()));
        }
        self.candidate = Some(image);
        self.state = WriteState::ImageValidated;
        Ok(())
    }

    pub fn confirm_recovery(&mut self, plan: String) -> Result<()> {
        self.require_state(&[WriteState::ImageValidated])?;
        if plan.trim().is_empty() {
            self.block("no recovery method".into());
            return Err(BiosToolError::NoRecovery);
        }
        self.recovery_plan = Some(plan);
        self.state = WriteState::RecoveryConfirmed;
        Ok(())
    }

    pub fn authorize_write(&mut self, confirm_phrase: &str) -> Result<()> {
        self.require_state(&[WriteState::RecoveryConfirmed])?;
        let expected = self
            .board
            .as_ref()
            .map(|b| b.exact_identity_string())
            .unwrap_or_default();
        if confirm_phrase.trim() != expected {
            self.block(format!(
                "confirmation phrase mismatch: got {:?}, expected exact board identity {:?}",
                confirm_phrase, expected
            ));
            return Err(BiosToolError::WriteBlocked(
                "confirmation phrase does not match exact board identity".into(),
            ));
        }
        if !self.backend_ok {
            self.block("backend not ready / unsupported".into());
            return Err(BiosToolError::BackendUnsupported("backend_ok=false".into()));
        }
        self.state = WriteState::WriteAuthorized;
        Ok(())
    }

    pub fn begin_writing(&mut self) -> Result<()> {
        self.require_state(&[WriteState::WriteAuthorized])?;
        if self.dry_run {
            return Err(BiosToolError::DryRun);
        }
        self.state = WriteState::Writing;
        Ok(())
    }

    pub fn complete_readback(&mut self, ok: bool) -> Result<()> {
        self.require_state(&[WriteState::Writing])?;
        if !ok {
            self.block("post-write read-back verification failed".into());
            return Err(BiosToolError::PostWriteVerification("hash mismatch".into()));
        }
        self.state = WriteState::ReadbackVerified;
        Ok(())
    }

    fn require_state(&mut self, allowed: &[WriteState]) -> Result<()> {
        if matches!(self.state, WriteState::Blocked { .. }) {
            return Err(BiosToolError::WriteBlocked(format!("{:?}", self.state)));
        }
        let ok = allowed.iter().any(|s| {
            std::mem::discriminant(s) == std::mem::discriminant(&self.state)
        });
        if !ok {
            let msg = format!("invalid transition from {:?}", self.state);
            self.block(msg.clone());
            return Err(BiosToolError::WriteBlocked(msg));
        }
        Ok(())
    }

    fn block(&mut self, reason: String) {
        self.state = WriteState::Blocked { reason };
    }

    pub fn is_authorized_for_write(&self) -> bool {
        matches!(self.state, WriteState::WriteAuthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BoardIdentity, ChipIdentity, ImageMeta, ImageKind, IdentityConfidence, ChipConfidence};

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

    #[test]
    fn wrong_confirm_phrase_blocks() {
        let mut ctx = SafetyContext::new(false);
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
        ctx.identify(board, chip).unwrap();
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
        assert!(ctx.authorize_write("wrong phrase").is_err());
        assert!(matches!(ctx.state, WriteState::Blocked { .. }));
    }
}
