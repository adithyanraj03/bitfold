//! Canonical Huffman coding (RFC 1951 §3.2.2, §3.2.5, §3.2.6, §3.2.7).
//!
//! * `canonical_codes` implements the RFC's bl_count / next_code /
//!   assign algorithm verbatim.
//! * The fixed literal/length and distance tables (§3.2.6) are built by
//!   running the same algorithm on the RFC's fixed length sequences.
//! * The length codes 257..285 and distance codes 0..29 with their extra
//!   bits (§3.2.5) are the two lookup tables.
//! * `encode_code_lengths` / `decode_code_lengths` implement the 19-symbol
//!   code-length alphabet with the 16/17/18 repeat runs (§3.2.7).

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
    debug_assert!(extra < 1 << LEN_EXTRA[i], "extra {extra} does not fit code {i}");
    (i as u32, extra)
}

/// Pick the distance code for a backward distance (1..=32768).
#[must_use]
pub fn distance_code(dist: u32) -> (u32, u32) {
    debug_assert!((1..=32768).contains(&dist));
    // Largest i with DIST_BASE[i] <= dist. Code 29 (base 24577, 13 extra
    // bits) covers 24577..32768; every extra value fits its field.
    let mut lo = 0usize;
    let mut hi = 29usize;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if DIST_BASE[mid] <= dist {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let extra = dist - DIST_BASE[lo];
    debug_assert!(extra < 1 << DIST_EXTRA[lo], "extra {extra} does not fit code {lo}");
    (lo as u32, extra)
}

/// A decoded dynamic code table: code lengths, canonical codes, and a
/// fast lookup (length, code) -> symbol.
#[derive(Debug, Clone)]
pub struct CodeTable {
    pub lengths: Vec<u8>,
    pub codes: Vec<u32>,
    /// `lookup[len][code]` = symbol, or `u16::MAX` if no such code exists.
    lookup: Vec<Vec<u16>>,
}

impl CodeTable {
    /// Build a table from bit lengths (canonical assignment).
    pub fn build(lengths: &[u8]) -> CodeTable {
        let codes = canonical_codes(lengths, 15);
        let max_len = lengths.iter().copied().max().unwrap_or(0) as usize;
        let mut lookup: Vec<Vec<u16>> = (0..=max_len.max(1))
            .map(|l| vec![u16::MAX; 1 << l])
            .collect();
        for (i, (&l, &c)) in lengths.iter().zip(codes.iter()).enumerate() {
            if l != 0 {
                lookup[l as usize][c as usize] = i as u16;
            }
        }
        CodeTable {
            lengths: lengths.to_vec(),
            codes,
            lookup,
        }
    }

    /// The maximum code length in the table (0 if all lengths are 0).
    pub fn max_length(&self) -> u32 {
        self.lengths.iter().copied().max().unwrap_or(0) as u32
    }

    /// Decode one symbol given its (length, code) pair.
    pub fn decode(&self, code: u32, len: u32) -> Option<usize> {
        if len == 0 || len >= self.lookup.len() as u32 {
            return None;
        }
        match self.lookup[len as usize][code as usize] {
            u16::MAX => None,
            s => Some(s as usize),
        }
    }
}

/// Encode a sequence of code lengths (257..286 lit/length + 1..32
/// distance, concatenated) into the 19-symbol code-length alphabet,
/// using the 16/17/18 repeat runs (RFC §3.2.7).
///
/// Returns the symbol sequence (values 0..18) and, for each 16/17/18
/// symbol, the repeat-count bits (2/3/7-bit fields, 0 for others).
#[must_use]
pub fn encode_code_lengths(lengths: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let mut syms: Vec<u32> = Vec::new();
    let mut reps: Vec<u32> = Vec::new();
    let mut i = 0usize;
    let n = lengths.len();
    while i < n {
        let l = lengths[i];
        if l == 0 {
            // Count the run of zeros.
            let mut run = 0;
            while i + run < n && lengths[i + run] == 0 {
                run += 1;
            }
            if run < 3 {
                for _ in 0..run {
                    syms.push(0);
                    reps.push(0);
                }
            } else if run <= 10 {
                syms.push(17);
                reps.push((run - 3) as u32); // 3 bits
            } else {
                // 18 covers 11..138; a longer run needs several.
                let mut rest = run;
                while rest >= 3 {
                    let take = if rest >= 11 {
                        (rest.min(138)) as u32
                    } else {
                        0
                    };
                    if take >= 11 {
                        syms.push(18);
                        reps.push(take - 11); // 7 bits
                        rest -= take as usize;
                    } else if rest >= 3 && rest <= 10 {
                        syms.push(17);
                        reps.push((rest - 3) as u32);
                        rest = 0;
                    } else {
                        break;
                    }
                }
                if rest > 0 && rest < 3 {
                    for _ in 0..rest {
                        syms.push(0);
                        reps.push(0);
                    }
                }
            }
            i += run;
        } else {
            // Literal length symbol.
            syms.push(l as u32);
            reps.push(0);
            i += 1;
            // Try to extend a run of the same length with 16.
            if l <= 15 {
                let mut run = 0;
                while i + run < n && lengths[i + run] == l {
                    run += 1;
                }
                if run >= 4 {
                    // `run` is counted after the first literal was already
                    // emitted, so it is the number of *remaining* copies.
                    let mut rest = run;
                    while rest >= 3 {
                        let take = rest.min(6);
                        syms.push(16);
                        reps.push((take - 3) as u32); // 2 bits
                        rest -= take;
                    }
                    if rest > 0 {
                        for _ in 0..rest {
                            syms.push(l as u32);
                            reps.push(0);
                        }
                    }
                    i += run;
                }
            }
        }
    }
    (syms, reps)
}

/// Decode the code-length sequence from the 19-symbol alphabet.
///
/// `read_sym` must return the next symbol (0..18); `read_bits` the next
/// `n` bits LSB-first. Returns the reconstructed length sequence
/// (`expected` long); errors on overrun or a 16-run at sequence start.
pub fn decode_code_lengths(
    mut read_sym: impl FnMut() -> Result<u32, crate::bits::BitError>,
    mut read_bits: impl FnMut(u32) -> Result<u64, crate::bits::BitError>,
    expected: usize,
) -> Result<Vec<u8>, crate::bits::BitError> {
    let mut out: Vec<u8> = Vec::with_capacity(expected);
    while out.len() < expected {
        let s = read_sym()? as usize;
        match s {
            0..=15 => out.push(s as u8),
            16 => {
                if out.is_empty() {
                    return Err(crate::bits::BitError::Exhausted);
                }
                let prev = *out.last().unwrap();
                let rep = read_bits(2)? as usize + 3; // 3..6
                for _ in 0..rep {
                    if out.len() < expected {
                        out.push(prev);
                    }
                }
            }
            17 => {
                let rep = read_bits(3)? as usize + 3; // 3..10
                for _ in 0..rep {
                    if out.len() < expected {
                        out.push(0);
                    }
                }
            }
            18 => {
                let rep = read_bits(7)? as usize + 11; // 11..138
                for _ in 0..rep {
                    if out.len() < expected {
                        out.push(0);
                    }
                }
            }
            _ => return Err(crate::bits::BitError::Exhausted),
        }
    }
    Ok(out)
}
