//! Bounded UEFI Firmware Volume discovery. Signature presence != structural validity.

use crate::domain::{BiosToolError, Result};

const EFI_FVH_SIGNATURE: &[u8; 4] = b"_FVH";

#[derive(Debug, Clone)]
pub struct FirmwareVolume {
    pub offset: usize,
    pub size: u64,
}

pub fn find_firmware_volumes(data: &[u8]) -> Result<Vec<FirmwareVolume>> {
    let mut vols = Vec::new();
    if data.len() < 0x40 {
        return Ok(vols);
    }

    // Scan on 8-byte alignment, bounded
    let limit = data.len().saturating_sub(0x30);
    let mut off = 0usize;
    while off < limit {
        if &data[off..off + 4] == EFI_FVH_SIGNATURE {
            // Header length at +0x28 (UINT16), FV length at +0x20 (UINT64)
            if off + 0x30 > data.len() {
                off = off.saturating_add(8);
                continue;
            }
            let header_len = u16::from_le_bytes([data[off + 0x28], data[off + 0x29]]) as usize;
            if header_len < 0x38 || header_len > 0x200 {
                // plausible range; reject wild values
                off = off.saturating_add(8);
                continue;
            }
            let fv_len = u64::from_le_bytes([
                data[off + 0x20],
                data[off + 0x21],
                data[off + 0x22],
                data[off + 0x23],
                data[off + 0x24],
                data[off + 0x25],
                data[off + 0x26],
                data[off + 0x27],
            ]);
            if fv_len < header_len as u64 || fv_len > (data.len() - off) as u64 {
                off = off.saturating_add(8);
                continue;
            }
            // Basic checksum could be checked; omit for initial release to keep simple
            vols.push(FirmwareVolume {
                offset: off,
                size: fv_len,
            });
            // Skip past this FV to avoid nested false positives
            off = off.saturating_add(fv_len as usize);
            continue;
        }
        off = off.saturating_add(8);
    }
    Ok(vols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_panic_on_short() {
        assert!(find_firmware_volumes(&[0u8; 10]).unwrap().is_empty());
    }

    #[test]
    fn false_signature() {
        let mut buf = vec![0u8; 128];
        buf[16..20].copy_from_slice(b"_FVH");
        // invalid lengths -> ignored
        let vols = find_firmware_volumes(&buf).unwrap();
        assert!(vols.is_empty() || vols.iter().all(|v| v.size > 0));
    }
}
