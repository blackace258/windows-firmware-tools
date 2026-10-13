//! Acquire the published SMBIOS table.
//! On Windows: GetSystemFirmwareTable with RSMB provider.
//! Treat all firmware bytes as untrusted. Keep raw acquisition separate from parsing.

use crate::error::{Result, SmbiosError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Provider signature for raw SMBIOS table ('RSMB' as u32 little-endian).
const RSMB: u32 = 0x5253_4D42; // 'RSMB'

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSmbiosTable {
    /// Exact bytes returned by the OS (including any header the API prepends).
    pub raw_bytes: Vec<u8>,
    /// SHA-256 of raw_bytes for reproducibility.
    pub sha256: String,
    /// Length reported / used.
    pub length: usize,
    /// Acquisition source description.
    pub source: String,
    /// UTC timestamp of acquisition.
    pub acquired_at: String,
    /// Note: this is the *published* table only; it does not identify the
    /// persistent firmware source of any field.
    pub provenance_note: String,
}

impl RawSmbiosTable {
    pub fn hash(data: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(data);
        hex::encode(h.finalize())
    }
}

/// Read the published SMBIOS table via the platform interface.
pub fn read_published_table() -> Result<RawSmbiosTable> {
    #[cfg(windows)]
    {
        read_windows_rsmb()
    }
    #[cfg(not(windows))]
    {
        Err(SmbiosError::Unsupported(
            "GetSystemFirmwareTable is Windows-only. On non-Windows hosts use --from-file with a captured table.".into(),
        ))
    }
}

/// Load a previously saved raw table (for offline parse/compare).
pub fn load_from_file(path: &std::path::Path) -> Result<RawSmbiosTable> {
    let raw_bytes = std::fs::read(path)?;
    if raw_bytes.is_empty() {
        return Err(SmbiosError::Malformed("empty file".into()));
    }
    let sha256 = RawSmbiosTable::hash(&raw_bytes);
    Ok(RawSmbiosTable {
        length: raw_bytes.len(),
        raw_bytes,
        sha256,
        source: format!("file:{}", path.display()),
        acquired_at: chrono::Utc::now().to_rfc3339(),
        provenance_note: "Loaded from file. This is a published-table snapshot only; persistent firmware source is not known from these bytes alone.".into(),
    })
}

pub fn save_raw(table: &RawSmbiosTable, path: &std::path::Path) -> Result<()> {
    std::fs::write(path, &table.raw_bytes)?;
    Ok(())
}

#[cfg(windows)]
fn read_windows_rsmb() -> Result<RawSmbiosTable> {
    use windows::Win32::System::SystemInformation::GetSystemFirmwareTable;

    // First call: size query
    let size = unsafe { GetSystemFirmwareTable(RSMB, 0, None) };
    if size == 0 {
        return Err(SmbiosError::WinApi(
            "GetSystemFirmwareTable size query returned 0 (table unavailable or access denied)".into(),
        ));
    }

    let mut buf = vec![0u8; size as usize];
    let written = unsafe { GetSystemFirmwareTable(RSMB, 0, Some(&mut buf)) };
    if written == 0 || written > size {
        return Err(SmbiosError::WinApi(format!(
            "GetSystemFirmwareTable read failed or size mismatch (requested {}, got {})",
            size, written
        )));
    }
    buf.truncate(written as usize);

    let sha256 = RawSmbiosTable::hash(&buf);
    Ok(RawSmbiosTable {
        length: buf.len(),
        raw_bytes: buf,
        sha256,
        source: "GetSystemFirmwareTable(RSMB)".into(),
        acquired_at: chrono::Utc::now().to_rfc3339(),
        provenance_note: "Published SMBIOS table only. Changing a local copy of this buffer does not modify firmware-managed persistent storage. Persistence after reboot requires a supported platform write path (not implemented in this read-only tool).".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_stable() {
        let d = b"test";
        assert_eq!(RawSmbiosTable::hash(d), RawSmbiosTable::hash(d));
    }
}
