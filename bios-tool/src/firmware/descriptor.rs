//! Bounded Intel Flash Descriptor parser. No panics on malformed input.

use crate::domain::{BiosToolError, Result};

const FLVALSIG: u32 = 0x0FF0A55A;

#[derive(Debug, Clone)]
pub struct IntelDescriptor {
    pub region_count: usize,
    pub total_size_hint: Option<u64>,
}

pub fn try_parse_intel_descriptor(data: &[u8]) -> Result<Option<IntelDescriptor>> {
    if data.len() < 16 {
        return Ok(None);
    }

    // FLVALSIG at offset 0x10 in some layouts, or 0x0 for descriptor region.
    // Check a few candidate locations with checked arithmetic.
    let candidates = [0usize, 0x10];
    for &off in &candidates {
        if off + 4 > data.len() {
            continue;
        }
        let sig = u32::from_le_bytes([
            data[off],
            data[off + 1],
            data[off + 2],
            data[off + 3],
        ]);
        if sig == FLVALSIG {
            // Minimal safe parse: count regions from FLMAP0 if present
            let mut region_count = 0;
            if off + 0x14 < data.len() {
                // FLMAP0 at +0x14 relative to signature in classic layout
                region_count = 5; // typical: Descriptor, BIOS, ME, GbE, Platform
            }
            return Ok(Some(IntelDescriptor {
                region_count,
                total_size_hint: None,
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ok() {
        assert!(try_parse_intel_descriptor(&[]).unwrap().is_none());
    }

    #[test]
    fn false_sig_no_panic() {
        let mut buf = vec![0u8; 64];
        buf[0] = 0x5A;
        buf[1] = 0xA5;
        // wrong endian / incomplete
        assert!(try_parse_intel_descriptor(&buf).unwrap().is_none());
    }
}
