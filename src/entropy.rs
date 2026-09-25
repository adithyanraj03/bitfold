//! The u128 fixed-point Shannon entropy ledger.
//!
//! Each corpus part (and the total) is measured by its 256-bin symbol
//! frequency histogram. The entropy `H = -sum p_i log2 p_i` is computed
//! once in `f64` (the only transcendental) and then frozen into a
//! **u128 fixed-point** value scaled by `2^FRACTION` so that every
//! downstream comparison — achieved bits vs. the entropy bound, the
//! "wasted bits" gap, per-part deltas — is exact integer arithmetic with
//! no float drift.
//!
//! `FRACTION = 40` gives sub-bit resolution of ~2^-40, far below any
//! meaningful reporting precision.

/// Fixed-point fraction bits: entropy and bit counts are stored as
/// `value * 2^FRACTION`.
pub const FRACTION: u32 = 40;

/// `2^FRACTION` as a `u128`.
pub const SCALE: u128 = 1 << FRACTION;

/// One part of the corpus ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct PartLedger {
    pub name: &'static str,
    /// Number of bytes in the part.
    pub n_bytes: usize,
    /// Shannon entropy in bits/byte, fixed-point (`H * 2^40`).
    pub entropy_fp: u128,
    /// Shannon entropy in bits/byte (float, for display).
    pub entropy_bits_per_byte: f64,
    /// The theoretical best ratio: 8 / H (float, for display).
    pub ratio_bound: f64,
    /// Number of distinct symbols observed.
    pub n_symbols: usize,
    /// Top 8 symbols by frequency: (byte, count).
    pub top: [(u8, u64); 8],
}

/// The full ledger: per-part plus the combined total.
#[derive(Debug, Clone, PartialEq)]
pub struct Ledger {
    pub parts: Vec<PartLedger>,
    /// Total bytes.
    pub n_bytes: usize,
    /// Total entropy in bits, fixed-point (bits * 2^40) — the sum over
    /// all bytes, i.e. `n_bytes * 2^40 * H` for the combined histogram.
    pub entropy_bits_fp: u128,
    /// Combined entropy in bits/byte (float, for display).
    pub entropy_bits_per_byte: f64,
    /// Combined theoretical best ratio: 8 / H.
    pub ratio_bound: f64,
}

/// Count bytes into a 256-bin histogram.
fn histogram(data: &[u8]) -> [u64; 256] {
    let mut h = [0u64; 256];
    for &b in data {
        h[b as usize] += 1;
    }
    h
}

/// Shannon entropy in bits/byte from a histogram (0.0 for empty input).
fn entropy_bits_per_byte(h: &[u64; 256]) -> f64 {
    let n = h.iter().sum::<u64>() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let mut e = 0.0f64;
    for &c in h.iter() {
        if c > 0 {
            let p = c as f64 / n;
            e -= p * p.log2();
        }
    }
    e
}

/// Fixed-point (bits * 2^40) total entropy for a byte slice.
fn entropy_bits_fp(data: &[u8]) -> u128 {
    let h = histogram(data);
    let e = entropy_bits_per_byte(&h);
    ((e * data.len() as f64) * (SCALE as f64)) as u128
}

/// Measure one named part.
fn part(name: &'static str, data: &[u8]) -> PartLedger {
    let h = histogram(data);
    let e = entropy_bits_per_byte(&h);
    let mut counts: Vec<(u8, u64)> = (0..=255u8)
        .map(|b| (b, h[b as usize]))
        .filter(|&(_, c)| c > 0)
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut top = [(0u8, 0u64); 8];
    for i in 0..8.min(counts.len()) {
        top[i] = counts[i];
    }
    PartLedger {
        name,
        n_bytes: data.len(),
        entropy_fp: ((e * (SCALE as f64)) as u128),
        entropy_bits_per_byte: e,
        ratio_bound: if e > 0.0 { 8.0 / e } else { 0.0 },
        n_symbols: counts.len(),
        top,
    }
}

/// Build the ledger over a set of named parts and their concatenation.
pub fn ledger(parts: &[(&'static str, &[u8])]) -> Ledger {
    let part_ledgers: Vec<PartLedger> = parts.iter().map(|(n, d)| part(n, d)).collect();
    let total: Vec<u8> = parts.iter().flat_map(|(_, d)| d.iter().copied()).collect();
    let h = histogram(&total);
    let e = entropy_bits_per_byte(&h);
    Ledger {
        parts: part_ledgers,
        n_bytes: total.len(),
        entropy_bits_fp: entropy_bits_fp(&total),
        entropy_bits_per_byte: e,
        ratio_bound: if e > 0.0 { 8.0 / e } else { 0.0 },
    }
}

/// The fixed-point "wasted bits" gap: `achieved_bits` (the encoder's
/// logical DEFLATE bit count) minus the entropy bound, in `bits * 2^40`
/// units. Positive = above the bound (always, by prefix-code overhead);
/// a smaller gap is better.
#[must_use]
pub fn wasted_bits_fp(achieved_bits: u64, ledger: &Ledger) -> u128 {
    let achieved_fp = (achieved_bits as u128) * SCALE;
    if achieved_fp >= ledger.entropy_bits_fp {
        achieved_fp - ledger.entropy_bits_fp
    } else {
        0
    }
}

/// The fixed-point "efficiency": entropy bound / achieved bits, scaled
/// by 2^40 (1.0 * 2^40 = 100% of the theoretical limit).
#[must_use]
pub fn efficiency_fp(achieved_bits: u64, ledger: &Ledger) -> u128 {
    if achieved_bits == 0 {
        return SCALE;
    }
    let achieved_fp = (achieved_bits as u128) * SCALE;
    // (entropy_bound / achieved) * 2^40.
    ledger.entropy_bits_fp * SCALE / achieved_fp
}
