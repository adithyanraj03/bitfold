//! The verification corpus: three public-domain parts, committed to the
//! repo and embedded at compile time (`include_bytes!`, so no runtime
//! disk I/O and no environmental input).
//!
//! * `rfc1951.txt` — the DEFLATE specification itself (prose: high
//!   redundancy, compresses strongly).
//! * `rfc8439.txt` — the ChaCha20-Poly1305 specification (prose + hex
//!   dumps: mixed redundancy).
//! * `b0000796.txt` — the OEIS b-file for A000796, 100,000 digits of pi
//!   in `(index, digit)` lines (structured digits: the entropy floor of
//!   the study — each digit line is a small alphabet).
//!
//! The parts are deliberately heterogeneous: the entropy ledger (and the
//! encoder's block-type choices) are the most informative when compress-
//! ible and near-incompressible data sit side by side.

/// Part 1: the DEFLATE specification (RFC 1951), 36,944 bytes.
pub const RFC1951: &[u8] = include_bytes!("../corpus/rfc1951.txt");
/// Part 2: the ChaCha20-Poly1305 specification (RFC 8439), 88,847 bytes.
pub const RFC8439: &[u8] = include_bytes!("../corpus/rfc8439.txt");
/// Part 3: OEIS b-file A000796 (digits of pi), 148,898 bytes.
pub const OEIS_B000796: &[u8] = include_bytes!("../corpus/b0000796.txt");

/// One named corpus part.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub name: &'static str,
    pub data: &'static [u8],
}

/// The three parts in canonical order.
pub const PARTS: [Part; 3] = [
    Part {
        name: "rfc1951",
        data: RFC1951,
    },
    Part {
        name: "rfc8439",
        data: RFC8439,
    },
    Part {
        name: "b0000796",
        data: OEIS_B000796,
    },
];

/// Assemble the full corpus: the parts concatenated in canonical order
/// (deterministic; 274,689 bytes).
#[must_use]
pub fn assemble() -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(PARTS.iter().map(|p| p.data.len()).sum());
    for p in PARTS {
        out.extend_from_slice(p.data);
    }
    out
}

/// The total corpus size in bytes (274,689).
pub const TOTAL_BYTES: usize = RFC1951.len() + RFC8439.len() + OEIS_B000796.len();
