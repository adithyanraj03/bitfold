//! CRC-32 (IEEE 802.3 / zlib): reflected polynomial `0xEDB88320`,
//! initial value `0xFFFFFFFF`, final XOR `0xFFFFFFFF`.
//!
//! Verified against the full zlib test battery (the `hash_tests` array of
//! zlib's `hash_test_strings_p.h`, 168 vectors with non-zero initial
//! states, boundary lengths, and the `i % 256` pattern buffer up to
//! 615,336 bytes) — see `kats.rs`.

/// Reflected CRC-32 table (polynomial 0x04C11DB7 reflected = 0xEDB88320).
const TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

/// One-shot CRC-32 starting from `initial` (use 0 for a fresh stream).
///
/// `crc32(0, data)` is the standard CRC-32 of `data`.
pub fn crc32(initial: u32, data: &[u8]) -> u32 {
    let mut crc = initial ^ 0xFFFF_FFFF;
    for &b in data {
        crc = TABLE[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Standard CRC-32 (fresh state) of `data`.
#[must_use]
pub fn crc32_fresh(data: &[u8]) -> u32 {
    crc32(0, data)
}

/// The 256-entry table (exposed for golden-file KATs).
pub const TABLE_256: [u32; 256] = TABLE;
