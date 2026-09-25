//! Entropy ledger tests: known entropies, fixed-point consistency, and
//! the wasted/efficiency relations.

use bitfold::entropy::{efficiency_fp, ledger, wasted_bits_fp, SCALE};

#[test]
fn constant_stream_has_zero_entropy() {
    let d: Vec<u8> = vec![b'x'; 10_000];
    let ld = ledger(&[("a", &d)]);
    assert_eq!(ld.entropy_bits_per_byte, 0.0);
    assert_eq!(ld.entropy_bits_fp, 0);
    assert_eq!(ld.parts[0].n_symbols, 1);
    assert_eq!(ld.parts[0].top[0], (b'x', 10_000));
}

#[test]
fn two_equal_symbols_give_one_bit() {
    let mut d = Vec::with_capacity(1024);
    for _ in 0..512 {
        d.push(b'a');
        d.push(b'b');
    }
    let ld = ledger(&[("ab", &d)]);
    let h = ld.entropy_bits_per_byte;
    assert!((h - 1.0).abs() < 1e-12, "H = {h}");
    assert!((ld.ratio_bound - 8.0).abs() < 1e-12);
}

#[test]
fn uniform_256_approaches_eight_bits() {
    // 256 copies of each byte value (rotated per round): exactly uniform
    // -> H = 8.
    let mut d = Vec::with_capacity(256 * 256);
    for round in 0..256usize {
        for b in 0u8..=255u8 {
            d.push(b.wrapping_add(round as u8));
        }
    }
    let ld = ledger(&[("uni", &d)]);
    assert!((ld.entropy_bits_per_byte - 8.0).abs() < 1e-9, "H = {}", ld.entropy_bits_per_byte);
    assert_eq!(ld.parts[0].n_symbols, 256);
}

#[test]
fn ledger_total_consistent() {
    let a = b"aaaabbbbccccdddd";
    let b = b"01234567890123456789";
    let ld = ledger(&[("a", a), ("b", b)]);
    assert_eq!(ld.n_bytes, a.len() + b.len());
    let h = ld.entropy_bits_per_byte;
    // (The combined H is NOT bounded by the parts' H: mixing distinct
    // alphabets adds symbol-identity information — this case computes
    // 3.73 bits/byte against parts at 2.0 and 3.32.)
    // Fixed-point total = per-byte H * n, within one ulp of the scaled
    // float (the only float in the pipeline).
    let expect = (h * ld.n_bytes as f64 * SCALE as f64) as u128;
    let d = (ld.entropy_bits_fp as i128 - expect as i128).abs();
    assert!(d <= 1, "fp drift {d}");
}

/// 9999 'q' + one 'r': tiny but strictly positive entropy, so the
/// wasted/efficiency relations are exercised away from the degenerate
/// zero-entropy case.
fn near_constant() -> Vec<u8> {
    let mut d = vec![b'q'; 9999];
    d.push(b'r');
    d
}

#[test]
fn wasted_bits_clamps_below_bound() {
    let d = near_constant();
    let ld = ledger(&[("q", &d)]);
    assert!(ld.entropy_bits_per_byte > 0.0);
    // Far above the bound: the gap is the fixed-point difference.
    let big = 5000u64;
    let wasted = wasted_bits_fp(big, &ld);
    let expect = (big as u128) * SCALE - ld.entropy_bits_fp;
    assert_eq!(wasted, expect, "wasted must be the exact fp gap");
    // Below the bound: clamps to zero (the LZ77 beat-the-memoryless case).
    assert_eq!(wasted_bits_fp(1, &ld), 0);
}

#[test]
fn efficiency_relations() {
    let d = near_constant();
    let base = ledger(&[("q", &d)]);
    // Zero achieved bits is defined as exactly 100% (no data, no waste).
    assert_eq!(efficiency_fp(0, &base), SCALE);
    // Exact identities via synthetic integer-multiple bounds (the Ledger
    // fields are public, so we pin `entropy_bits_fp` to exact multiples).
    let x = 3u64;
    let mut ld = base.clone();
    ld.entropy_bits_fp = x as u128 * SCALE; // bound == achieved -> 100%
    assert_eq!(efficiency_fp(x, &ld), SCALE);
    ld.entropy_bits_fp = 2 * x as u128 * SCALE; // 2x bound -> 200%
    assert_eq!(efficiency_fp(x, &ld), 2 * SCALE);
    ld.entropy_bits_fp = 3 * SCALE; // 1.5x bound -> 150% (exact in fp)
    assert_eq!(efficiency_fp(2, &ld), 3 * SCALE / 2);
    // A below-bound achievement must read above 100%.
    ld.entropy_bits_fp = x as u128 * SCALE + SCALE / 4;
    assert!(efficiency_fp(x, &ld) > SCALE);
}

#[test]
fn top_symbols_sorted_desc() {
    let mut d = vec![b'z'; 100];
    d.extend(b"abcabc");
    let ld = ledger(&[("t", &d)]);
    let top = &ld.parts[0].top;
    assert_eq!(top[0].0, b'z');
    for i in 0..top.len() - 1 {
        assert!(top[i].1 >= top[i + 1].1);
    }
    assert_eq!(top[0].1, 100);
}

#[test]
fn ledger_is_deterministic() {
    let d = b"the quick brown fox jumps over the lazy dog";
    let a = ledger(&[("x", d)]);
    let b = ledger(&[("x", d)]);
    assert_eq!(a, b);
    assert_eq!(a.parts[0].entropy_fp, b.parts[0].entropy_fp);
}
