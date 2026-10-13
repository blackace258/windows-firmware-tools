//! Full-chip SPI dump via flashrom: repeated reads, hashing, metadata.
//! Read path only — writes go exclusively through programming::WriteCoordinator.

use crate::backend::FlashromBackend;
use crate::domain::{ChipIdentity, ImageMeta, BiosToolError, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpMetadata {
    pub utc_time: String,
    pub board_model: String,
    pub board_revision: String,
    pub programmer: String,
    pub chip: ChipIdentity,
    pub size: u64,
    pub read_hashes: Vec<String>,
    pub final_sha256: String,
    pub tool_version: String,
    pub flashrom_version: Option<String>,
    pub probe_ok: bool,
    pub notes: Vec<String>,
}

/// Read the entire SPI flash chip to `output`.
///
/// - Optionally probes the programmer first (non-fatal if probe fails).
/// - Performs `reads` full-chip reads; all must match byte-for-byte or the dump is refused.
/// - Writes the image only after consistency is confirmed.
/// - Never modifies the chip.
pub fn dump_full_chip(
    backend: &FlashromBackend,
    output: &Path,
    reads: u32,
    force: bool,
    board_model: &str,
    board_rev: &str,
) -> Result<DumpMetadata> {
    if output.exists() && !force {
        return Err(BiosToolError::OverwriteRefused(output.display().to_string()));
    }

    if reads < 1 {
        return Err(BiosToolError::Other("reads must be >= 1".into()));
    }

    let mut notes = Vec::new();
    let flashrom_ver = backend.version().ok();
    if let Some(ref v) = flashrom_ver {
        notes.push(format!("flashrom: {}", v));
    }
    notes.push(format!(
        "programmer={} ({})",
        backend.config.programmer,
        backend.programmer_windows_support_note()
    ));

    // Probe first (read-only). Failure is recorded but does not block dump;
    // operator must still confirm chip identity before any write.
    let (chip, probe_ok) = match backend.probe() {
        Ok((chip, result)) => {
            notes.push(format!("probe exit={}", result.exit_code));
            if let Some(ref raw) = chip.probe_raw {
                let excerpt: String = raw.chars().take(200).collect();
                notes.push(format!("probe excerpt: {}", excerpt));
            }
            (chip, true)
        }
        Err(e) => {
            notes.push(format!("probe failed (continuing to read): {:#}", e));
            (ChipIdentity::default(), false)
        }
    };

    let mut hashes = Vec::new();
    let mut last_data: Option<Vec<u8>> = None;
    let mut capacity: Option<u64> = None;

    for i in 0..reads {
        let tmp = NamedTempFile::new().map_err(BiosToolError::Io)?;
        let tmp_path = tmp.path().to_path_buf();
        let result = backend.read_chip(&tmp_path)?;
        if result.exit_code != 0 {
            return Err(BiosToolError::Process(format!(
                "flashrom read failed exit={}: {}",
                result.exit_code, result.stderr
            )));
        }
        let data = std::fs::read(&tmp_path).map_err(BiosToolError::Io)?;
        let size = data.len() as u64;
        if size == 0 {
            return Err(BiosToolError::Other(
                "flashrom produced empty read — check programmer connection and Windows drivers"
                    .into(),
            ));
        }
        if let Some(c) = capacity {
            if size != c {
                return Err(BiosToolError::SizeMismatch {
                    expected: c,
                    actual: size,
                });
            }
        } else {
            capacity = Some(size);
        }
        let hash = ImageMeta::hash_bytes(&data);
        hashes.push(hash.clone());

        if let Some(ref prev) = last_data {
            if prev.as_slice() != data.as_slice() {
                return Err(BiosToolError::HashMismatch);
            }
        }
        last_data = Some(data);
        eprintln!("SPI read {}/{} size={} sha256={}", i + 1, reads, size, hash);
        let _ = result;
    }

    let data = last_data.ok_or_else(|| BiosToolError::Other("no data".into()))?;
    let final_hash = ImageMeta::hash_bytes(&data);
    let size = capacity.unwrap_or(0);

    // Prefer measured size over probe assumption when they differ
    let mut chip = chip;
    if chip.capacity_bytes == 0 || chip.capacity_bytes != size {
        if size > 0 {
            notes.push(format!(
                "setting chip capacity from measured read size: {} bytes",
                size
            ));
            chip.capacity_bytes = size;
        }
    }

    // Atomic-ish write of final image
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp_out = NamedTempFile::new_in(parent).map_err(BiosToolError::Io)?;
    tmp_out.write_all(&data).map_err(BiosToolError::Io)?;
    tmp_out
        .persist(output)
        .map_err(|e| BiosToolError::Io(e.error))?;

    Ok(DumpMetadata {
        utc_time: chrono::Utc::now().to_rfc3339(),
        board_model: board_model.to_string(),
        board_revision: board_rev.to_string(),
        programmer: backend.config.programmer.clone(),
        chip,
        size,
        read_hashes: hashes,
        final_sha256: final_hash,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        flashrom_version: flashrom_ver,
        probe_ok,
        notes,
    })
}

pub fn write_metadata_json(meta: &DumpMetadata, path: &Path) -> Result<()> {
    let f = std::fs::File::create(path).map_err(BiosToolError::Io)?;
    serde_json::to_writer_pretty(f, meta)?;
    Ok(())
}
