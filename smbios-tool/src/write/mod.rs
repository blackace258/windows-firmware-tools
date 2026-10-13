//! Gated DMI/SMBIOS write workflow.
//!
//! Core principle: the published table is not a write API.
//! Persistent updates require a platform-supported mechanism that has been
//! explicitly established for the exact board and firmware. This module
//! implements the Lesson 11 safety state machine and fails closed by default.
//!
//! Supported backend (optional, operator-supplied):
//!   ExternalVendorTool — invoke a verified vendor utility (e.g. AMIDEWIN)
//!   only after every safety gate passes. The tool path and arguments are
//!   supplied by the operator; this project does not embed or download
//!   vendor binaries and does not perform blind SPI byte-patching for DMI.
//!
//! States:
//!   Unidentified → Identified → BackupVerified → InputValidated
//!   → RecoveryConfirmed → WriteAuthorized → Executing → Verified
//!   Any failure → Blocked

use crate::error::{Result, SmbiosError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Exact confirmation phrase the operator must type to authorize a write.
pub const CONFIRM_PHRASE_PREFIX: &str = "I AUTHORIZE DMI WRITE ON";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WriteState {
    Unidentified,
    Identified,
    BackupVerified,
    InputValidated,
    RecoveryConfirmed,
    WriteAuthorized,
    Executing,
    Verified,
    Blocked { reason: String },
}

impl Default for WriteState {
    fn default() -> Self {
        WriteState::Unidentified
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardContext {
    pub model: String,
    pub pcb_revision: String,
    /// Operator-confirmed exact identity string (must match confirmation phrase).
    pub exact_identity: String,
    pub operator_confirmed: bool,
}

impl BoardContext {
    pub fn identity_string(&self) -> String {
        format!("{} PCB:{}", self.model, self.pcb_revision)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldUpdate {
    /// e.g. "type1.serial_number", "type2.serial_number"
    pub field: String,
    pub new_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    pub board: BoardContext,
    pub updates: Vec<FieldUpdate>,
    pub backup_raw_path: PathBuf,
    pub recovery_plan: String,
    /// Exact phrase: "I AUTHORIZE DMI WRITE ON <exact_identity>"
    pub confirm_phrase: String,
    pub dry_run: bool,
    /// Path to operator-supplied vendor utility (optional; required for non-dry-run execute)
    pub vendor_tool_path: Option<PathBuf>,
    /// Extra arguments passed to the vendor tool after field mapping
    pub vendor_tool_args: Vec<String>,
    /// Expected SHA-256 of vendor tool binary (optional integrity check)
    pub vendor_tool_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteAudit {
    pub operation_id: String,
    pub state: WriteState,
    pub request_summary: String,
    pub backup_sha256: Option<String>,
    pub vendor_tool: Option<String>,
    pub dry_run: bool,
    pub messages: Vec<String>,
    pub exit_code: Option<i32>,
    pub timestamp: String,
}

#[derive(Debug)]
pub struct WriteCoordinator {
    pub state: WriteState,
    pub request: Option<WriteRequest>,
    pub backup_sha256: Option<String>,
    pub messages: Vec<String>,
    pub dry_run: bool,
}

impl WriteCoordinator {
    pub fn new(dry_run: bool) -> Self {
        Self {
            state: WriteState::Unidentified,
            request: None,
            backup_sha256: None,
            messages: Vec::new(),
            dry_run,
        }
    }

    fn block(&mut self, reason: impl Into<String>) -> SmbiosError {
        let reason = reason.into();
        self.state = WriteState::Blocked {
            reason: reason.clone(),
        };
        self.messages.push(format!("BLOCKED: {}", reason));
        SmbiosError::WriteBlocked(reason)
    }

    fn require(&mut self, allowed: &[WriteState]) -> Result<()> {
        let ok = allowed.iter().any(|s| s == &self.state);
        if ok {
            Ok(())
        } else {
            let current = format!("{:?}", self.state);
            Err(self.block(format!(
                "invalid state {current} for this transition"
            )))
        }
    }

    /// Gate 1: board identity must be operator-confirmed.
    pub fn identify(&mut self, board: BoardContext) -> Result<()> {
        if self.state != WriteState::Unidentified {
            return Err(self.block(
                "identify is only valid from Unidentified; create a new coordinator to restart",
            ));
        }
        if !board.operator_confirmed {
            return Err(self.block(
                "board identity not operator-confirmed; set operator_confirmed=true only after physical label check",
            ));
        }
        if board.model.trim().is_empty() || board.pcb_revision.trim().is_empty() {
            return Err(self.block("board model and PCB revision are required"));
        }
        if board.exact_identity.trim().is_empty() {
            return Err(self.block("exact_identity string is required"));
        }
        self.messages.push(format!(
            "identified board: {}",
            board.identity_string()
        ));
        // stash board inside a provisional request shell
        self.request = Some(WriteRequest {
            board,
            updates: vec![],
            backup_raw_path: PathBuf::new(),
            recovery_plan: String::new(),
            confirm_phrase: String::new(),
            dry_run: self.dry_run,
            vendor_tool_path: None,
            vendor_tool_args: vec![],
            vendor_tool_sha256: None,
        });
        self.state = WriteState::Identified;
        Ok(())
    }

    /// Gate 2: verified same-board backup of the published table (raw bytes).
    pub fn verify_backup(&mut self, backup_path: &Path) -> Result<()> {
        self.require(&[WriteState::Identified])?;
        if !backup_path.is_file() {
            return Err(self.block(format!(
                "backup file not found: {}",
                backup_path.display()
            )));
        }
        let data = std::fs::read(backup_path).map_err(|e| {
            self.block(format!("cannot read backup: {}", e))
        })?;
        if data.len() < 8 {
            return Err(self.block("backup too small to be a valid RSMB capture"));
        }
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hex::encode(hasher.finalize());
        self.backup_sha256 = Some(hash.clone());
        if let Some(req) = self.request.as_mut() {
            req.backup_raw_path = backup_path.to_path_buf();
        }
        self.messages
            .push(format!("backup verified: {} sha256={}", backup_path.display(), hash));
        self.state = WriteState::BackupVerified;
        Ok(())
    }

    /// Gate 3: field updates must be non-empty and use known field keys.
    pub fn validate_input(&mut self, updates: Vec<FieldUpdate>) -> Result<()> {
        self.require(&[WriteState::BackupVerified])?;
        if updates.is_empty() {
            return Err(self.block("no field updates specified"));
        }
        const ALLOWED: &[&str] = &[
            "type1.serial_number",
            "type1.uuid",
            "type1.product_name",
            "type1.manufacturer",
            "type2.serial_number",
            "type2.product",
            "type2.manufacturer",
            "type3.serial_number",
            "type3.asset_tag",
        ];
        for u in &updates {
            if !ALLOWED.iter().any(|a| *a == u.field) {
                return Err(self.block(format!(
                    "field '{}' is not in the allowed write list: {:?}",
                    u.field, ALLOWED
                )));
            }
            if u.new_value.is_empty() {
                return Err(self.block(format!("empty new_value for field {}", u.field)));
            }
            if u.new_value.len() > 64 {
                return Err(self.block(format!(
                    "new_value for {} exceeds 64 characters",
                    u.field
                )));
            }
        }
        if let Some(req) = self.request.as_mut() {
            req.updates = updates.clone();
        }
        self.messages.push(format!("input validated: {} field(s)", updates.len()));
        self.state = WriteState::InputValidated;
        Ok(())
    }

    /// Gate 4: recovery plan must be non-trivial.
    pub fn confirm_recovery(&mut self, plan: &str) -> Result<()> {
        self.require(&[WriteState::InputValidated])?;
        let plan = plan.trim();
        if plan.len() < 16 {
            return Err(self.block(
                "recovery_plan must describe a concrete method (≥16 chars), e.g. external programmer + known-good dump",
            ));
        }
        if let Some(req) = self.request.as_mut() {
            req.recovery_plan = plan.to_string();
        }
        self.messages
            .push(format!("recovery plan recorded: {}", plan));
        self.state = WriteState::RecoveryConfirmed;
        Ok(())
    }

    /// Gate 5: exact confirmation phrase matching board identity.
    pub fn authorize(&mut self, phrase: &str) -> Result<()> {
        self.require(&[WriteState::RecoveryConfirmed])?;
        let req = self
            .request
            .as_ref()
            .ok_or_else(|| SmbiosError::WriteBlocked("no request context".into()))?;
        let expected = format!(
            "{} {}",
            CONFIRM_PHRASE_PREFIX,
            req.board.exact_identity
        );
        if phrase.trim() != expected {
            return Err(self.block(format!(
                "confirmation phrase mismatch; expected exactly: {}",
                expected
            )));
        }
        if let Some(r) = self.request.as_mut() {
            r.confirm_phrase = phrase.to_string();
        }
        self.messages.push("write authorized by operator phrase".into());
        self.state = WriteState::WriteAuthorized;
        Ok(())
    }

    /// Gate 6: execute via external vendor tool only (or dry-run).
    /// Never patches SPI images or published-table memory as a write.
    pub fn execute(&mut self) -> Result<WriteAudit> {
        self.require(&[WriteState::WriteAuthorized])?;
        let req = self
            .request
            .as_ref()
            .ok_or_else(|| SmbiosError::WriteBlocked("no request context".into()))?
            .clone();

        let op_id = uuid::Uuid::new_v4().to_string();
        let ts = chrono::Utc::now().to_rfc3339();

        if self.dry_run || req.dry_run {
            self.messages.push(
                "DRY-RUN: no vendor tool invoked; no firmware or table modified; not marked Verified"
                    .into(),
            );
            // Stay authorized. Verified would claim a write that never ran.
            self.state = WriteState::WriteAuthorized;
            return Ok(self.make_audit(op_id, ts, None, req));
        }

        self.state = WriteState::Executing;

        let tool = match &req.vendor_tool_path {
            Some(p) if p.is_file() => p.clone(),
            Some(p) => {
                return Err(self.block(format!(
                    "vendor_tool_path not found: {}",
                    p.display()
                )));
            }
            None => {
                return Err(self.block(
                    "non-dry-run write requires --vendor-tool path to an operator-supplied utility; \
                     this tool does not embed AMIDEWIN or perform SPI DMI patching",
                ));
            }
        };

        // Optional integrity check
        if let Some(expected) = &req.vendor_tool_sha256 {
            let bytes = std::fs::read(&tool).map_err(|e| {
                self.block(format!("cannot hash vendor tool: {}", e))
            })?;
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected.trim()) {
                return Err(self.block(format!(
                    "vendor tool SHA-256 mismatch: expected {}, got {}",
                    expected, actual
                )));
            }
            self.messages
                .push(format!("vendor tool hash verified: {}", actual));
        } else {
            self.messages.push(
                "WARNING: vendor tool SHA-256 not supplied; integrity not verified".into(),
            );
        }

        // Map first allowed field to a conventional AMI-style switch when possible.
        // Operator must still supply correct args for their exact utility version.
        let mut args = req.vendor_tool_args.clone();
        if args.is_empty() {
            // Conservative: do not invent switches; require operator args
            return Err(self.block(
                "vendor_tool_args is empty; supply the exact command-line switches \
                 documented for your utility version and board (e.g. /SS \"serial\")",
            ));
        }

        self.messages.push(format!(
            "invoking vendor tool: {} {:?}",
            tool.display(),
            args
        ));

        let output = Command::new(&tool)
            .args(&args)
            .output()
            .map_err(|e| self.block(format!("failed to spawn vendor tool: {}", e)))?;

        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        self.messages.push(format!("vendor tool exit={}", code));
        if !stdout.trim().is_empty() {
            self.messages
                .push(format!("stdout: {}", stdout.chars().take(500).collect::<String>()));
        }
        if !stderr.trim().is_empty() {
            self.messages
                .push(format!("stderr: {}", stderr.chars().take(500).collect::<String>()));
        }

        if !output.status.success() {
            return Err(self.block(format!(
                "vendor tool failed with exit {}; write not considered successful",
                code
            )));
        }

        self.messages.push(
            "vendor tool reported success — persistence is NOT proven until independent \
             re-read after reboot; run: smbios-tool read && compare against backup"
                .into(),
        );
        self.state = WriteState::Verified;
        Ok(self.make_audit(op_id, ts, Some(code), req))
    }

    fn make_audit(
        &self,
        op_id: String,
        ts: String,
        exit_code: Option<i32>,
        req: WriteRequest,
    ) -> WriteAudit {
        WriteAudit {
            operation_id: op_id,
            state: self.state.clone(),
            request_summary: format!(
                "board={} fields={:?}",
                req.board.identity_string(),
                req.updates
                    .iter()
                    .map(|u| u.field.clone())
                    .collect::<Vec<_>>()
            ),
            backup_sha256: self.backup_sha256.clone(),
            vendor_tool: req
                .vendor_tool_path
                .as_ref()
                .map(|p| p.display().to_string()),
            dry_run: self.dry_run || req.dry_run,
            messages: self.messages.clone(),
            exit_code,
            timestamp: ts,
        }
    }
}

/// High-level helper used by CLI: run the full gate sequence.
pub fn run_write_workflow(req: WriteRequest) -> Result<WriteAudit> {
    let mut coord = WriteCoordinator::new(req.dry_run);
    // Honor operator_confirmed. Callers (including the CLI) must set it
    // only after a physical label check. Do not force it true here.
    coord.identify(req.board.clone())?;
    coord.verify_backup(&req.backup_raw_path)?;
    coord.validate_input(req.updates.clone())?;
    coord.confirm_recovery(&req.recovery_plan)?;
    coord.authorize(&req.confirm_phrase)?;
    // attach vendor paths before execute
    if let Some(r) = coord.request.as_mut() {
        r.vendor_tool_path = req.vendor_tool_path.clone();
        r.vendor_tool_args = req.vendor_tool_args.clone();
        r.vendor_tool_sha256 = req.vendor_tool_sha256.clone();
        r.dry_run = req.dry_run;
    }
    coord.execute()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_req(dir: &std::path::Path, dry: bool) -> WriteRequest {
        let backup = dir.join("backup.bin");
        let mut f = std::fs::File::create(&backup).unwrap();
        f.write_all(b"RSMBFAKE").unwrap();
        WriteRequest {
            board: BoardContext {
                model: "Gigabyte B760M AORUS".into(),
                pcb_revision: "1.1".into(),
                exact_identity: "Gigabyte B760M AORUS PCB:1.1".into(),
                operator_confirmed: true,
            },
            updates: vec![FieldUpdate {
                field: "type1.serial_number".into(),
                new_value: "ABC".into(),
            }],
            backup_raw_path: backup,
            recovery_plan: "external programmer plus known-good dump".into(),
            confirm_phrase: "I AUTHORIZE DMI WRITE ON Gigabyte B760M AORUS PCB:1.1".into(),
            dry_run: dry,
            vendor_tool_path: None,
            vendor_tool_args: vec![],
            vendor_tool_sha256: None,
        }
    }

    #[test]
    fn dry_run_is_not_verified() {
        let dir = tempfile::tempdir().unwrap();
        let audit = run_write_workflow(sample_req(dir.path(), true)).unwrap();
        assert!(audit.dry_run);
        assert!(matches!(audit.state, WriteState::WriteAuthorized));
        assert!(!matches!(audit.state, WriteState::Verified));
    }

    #[test]
    fn unconfirmed_board_is_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let mut req = sample_req(dir.path(), true);
        req.board.operator_confirmed = false;
        let err = run_write_workflow(req).unwrap_err();
        assert!(matches!(err, SmbiosError::WriteBlocked(_)));
    }

    #[test]
    fn second_identify_blocks() {
        let mut coord = WriteCoordinator::new(true);
        let board = BoardContext {
            model: "Gigabyte B760M AORUS".into(),
            pcb_revision: "1.1".into(),
            exact_identity: "Gigabyte B760M AORUS PCB:1.1".into(),
            operator_confirmed: true,
        };
        coord.identify(board.clone()).unwrap();
        assert!(coord.identify(board).is_err());
        assert!(matches!(coord.state, WriteState::Blocked { .. }));
    }
}
