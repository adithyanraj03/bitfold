//! Canonical Huffman coding (RFC 1951 §3.2.2, §3.2.5, §3.2.6).
//!
//! * `canonical_codes` implements the RFC's bl_count / next_code /
//!   assign algorithm verbatim.
//! * The fixed literal/length and distance tables (§3.2.6) are built by
//!   running the same algorithm on the RFC's fixed length sequences.
//! * The length codes 257..285 and distance codes 0..29 with their extra
//!   bits (§3.2.5) are the two lookup tables.

/// The 19 code-length symbols in their on-the-wire order (RFC §3.2.7):
/// 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15.
pub const CODE_LENGTH_ORDER: [u8; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Assign canonical codes (read MSB-first) from bit lengths, using the
/// RFC §3.2.2 algorithm. Length 0 assigns no code.
///
/// `max_bits` is the maximum code length to process (1..=15).
#[must_use]
pub fn canonical_codes(lengths: &[u8], max_bits: u32) -> Vec<u32> {
    let max_bits = max_bits.min(15);
    let mut bl_count = [0u32; 16];
    for &l in lengths {
        if (1..=max_bits).contains(&(l as u32)) {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = [0u32; 16];
    let mut code = 0u32;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits as usize - 1]) << 1;
        next_code[bits as usize] = code;
    }
    let mut out = vec![0u32; lengths.len()];
    for (i, &l) in lengths.iter().enumerate() {
        if l != 0 {
            out[i] = next_code[l as usize];
            next_code[l as usize] += 1;
        }
    }
    out
}

/// The fixed literal/length code lengths (RFC §3.2.6):
/// 0-143: 8, 144-255: 9, 256-279: 7, 280-287: 8 (288 entries).
pub const FIXED_LIT_LENGTHS: [u8; 288] = fixed_lit_lengths();

const fn fixed_lit_lengths() -> [u8; 288] {
    let mut a = [0u8; 288];
    let mut i = 0;
    while i < 288 {
        a[i] = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
        i += 1;
    }
    a
}

/// The fixed distance code lengths (RFC §3.2.6): 5 for 0-29 (30 entries).
pub const FIXED_DIST_LENGTHS: [u8; 30] = [5u8; 30];

/// The length codes 257..285: base lengths and extra-bit counts
/// (RFC §3.2.5). Index `i` is code `257 + i`.
pub const LEN_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
pub const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// The distance codes 0..29: base distances and extra-bit counts
/// (RFC §3.2.5). Index `i` is code `i`.
pub const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
pub const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Pick the length code for a match length (3..258).
#[must_use]
pub fn length_code(len: u32) -> (u32, u32) {
    debug_assert!((3..=258).contains(&len));
    let i = if len == 258 { 28 } else {
        // Largest i with LEN_BASE[i] <= len.
        let mut lo = 0usize;
        let mut hi = 27usize; // codes 0..27 cover 3..257; code 28 is 258
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            if LEN_BASE[mid] <= len {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    };
    let extra = len - LEN_BASE[i];
    (i as u32, extra)
}

/// Pick the distance code for a backward distance (1..=32768).
#[must_use]
pub fn distance_code(dist: u32) -> (u32, u32) {
    debug_assert!((1..=32768).contains(&dist));
    // Largest i with DIST_BASE[i] <= dist.
    let mut lo = 0usize;
    let mut hi = DIST_BASE.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if DIST_BASE[mid] <= dist {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let extra = dist - DIST_BASE[lo];
    (lo as u32, extra)
}
