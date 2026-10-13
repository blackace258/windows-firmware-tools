//! Signature presence detection only. Never treat a hit as proof of valid structure.

pub fn find_signatures(data: &[u8]) -> Option<Vec<String>> {
    let mut found = Vec::new();
    let needles: &[(&[u8], &str)] = &[
        (b"_FVH", "UEFI_FVH"),
        (b"EFI PART", "GPT"),
        (b"\x5A\xA5\xF0\x0F", "Intel_FLVALSIG_BE"), // unusual
        (b"$MS$", "AMI_or_similar"),
        (b"CAPSULE", "Capsule_string"),
        (b"MX25L", "Macronix_string"),
    ];

    for (pat, name) in needles {
        if data.windows(pat.len()).any(|w| w == *pat) {
            found.push(name.to_string());
        }
    }

    // Intel FLVALSIG little-endian
    let flval = 0x0FF0A55Au32.to_le_bytes();
    if data.windows(4).any(|w| w == flval) {
        found.push("Intel_FLVALSIG".into());
    }

    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}
