//! Adler-32 (RFC 1950 §3.2): `a = 1 + sum of bytes (mod 65521)`,
//! `b = sum of running a's (mod 65521)`, result `(b << 16) | a`.
//!
//! The chunking follows zlib's `update_adler32` (`BASE = 5552`), but the
//! accumulators are `u64`, so the chunk-end reduction is exact even in
//! the worst case (all-0xFF data, maximum carry-in state) where the
//! 32-bit zlib form overflows.
//!
//! Verified against the full zlib test battery (168 vectors with non-zero
//! initial states, boundary lengths, and the `i % 256` pattern buffer up
//! to 615,336 bytes) — see `kats.rs`.

/// The Adler-32 modulus (prime).
pub const MOD: u32 = 65521;

/// zlib's chunk size (`BASE`): at most 5552 bytes between reductions.
pub const NMAX: usize = 5552;

/// Adler-32 with an explicit initial state.
///
/// The state is laid out `(b << 16) | a`; `adler32(1, data)` is the
/// standard Adler-32 (zlib's canonical fresh value is 1, meaning
/// `a = 1`, `b = 0`).
pub fn adler32(initial: u32, mut data: &[u8]) -> u32 {
    let mut a = (initial & 0xFFFF) as u64;
    let mut b = (initial >> 16) as u64;
    while data.len() >= NMAX {
        for &byte in &data[..NMAX] {
            a += byte as u64;
            b += a;
        }
        a %= MOD as u64;
        b %= MOD as u64;
        data = &data[NMAX..];
    }
    for &byte in data {
        a += byte as u64;
        b += a;
    }
    a %= MOD as u64;
    b %= MOD as u64;
    ((b << 16) | a) as u32
}

/// Standard Adler-32 (fresh state 1) of `data`.
#[must_use]
pub fn adler32_fresh(data: &[u8]) -> u32 {
    adler32(1, data)
}
