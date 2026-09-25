//! Huffman machinery tests: canonical assignment, fixed tables, code
//! pickers, and the code-length run coding.

use bitfold::bits::BitError;
use bitfold::deflate::huffman_lengths;
use bitfold::huff::{
    canonical_codes, decode_code_lengths, distance_code, encode_code_lengths, length_code,
    CodeTable, CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, FIXED_DIST_LENGTHS,
    FIXED_LIT_LENGTHS, LEN_BASE, LEN_EXTRA,
};

#[test]
fn canonical_rfc_example() {
    // RFC 1951 §3.2.2: lengths (3,3,3,3,3,2,4,4) for A..H.
    let c = canonical_codes(&[3, 3, 3, 3, 3, 2, 4, 4], 4);
    assert_eq!(c, [2, 3, 4, 5, 6, 0, 14, 15]);
    assert_eq!(c[0], 0b010); // A
    assert_eq!(c[1], 0b011); // B
    assert_eq!(c[5], 0b00); // F
    assert_eq!(c[6], 0b1110); // G
    assert_eq!(c[7], 0b1111); // H
}

#[test]
fn canonical_single_symbol() {
    let c = canonical_codes(&[1, 0, 0], 4);
    assert_eq!(c[0], 0);
    assert_eq!(c[1], 0);
    assert_eq!(c[2], 0);
}

#[test]
fn canonical_all_zero_lengths() {
    let c = canonical_codes(&[0, 0, 0, 0], 4);
    assert!(c.iter().all(|&x| x == 0));
}

#[test]
fn canonical_respects_max_bits_and_kraft() {
    // 8 symbols of length 3: Kraft-exact (8 * 2^-3 = 1).
    let c = canonical_codes(&[3u8; 8], 3);
    let mut seen = std::collections::HashSet::new();
    for x in &c {
        assert!(seen.insert(*x), "duplicate code {x:b}");
    }
    // 9 symbols of length 3 would be overfull: huffman_lengths refuses it.
    let freq: Vec<u64> = vec![1; 9];
    assert!(huffman_lengths(&freq, 3).is_none());
    // ...but with max_bits 4 it fits.
    let lens = huffman_lengths(&freq, 4).unwrap();
    let kraft: u32 = lens.iter().map(|&l| 1u32 << (4 - l as u32)).sum();
    assert_eq!(kraft, 1 << 4);
}

#[test]
fn fixed_lit_length_profile() {
    for s in 0..144 {
        assert_eq!(FIXED_LIT_LENGTHS[s], 8);
    }
    for s in 144..256 {
        assert_eq!(FIXED_LIT_LENGTHS[s], 9);
    }
    for s in 256..280 {
        assert_eq!(FIXED_LIT_LENGTHS[s], 7);
    }
    for s in 280..288 {
        assert_eq!(FIXED_LIT_LENGTHS[s], 8);
    }
}

#[test]
fn fixed_dist_lengths_all_five() {
    assert!(FIXED_DIST_LENGTHS.iter().all(|&l| l == 5));
}

#[test]
fn length_code_exhaustive_roundtrip() {
    for len in 3..=258u32 {
        let (lc, extra) = length_code(len);
        assert_eq!(LEN_BASE[lc as usize] + extra, len, "len {len}");
        assert!(extra < 1 << LEN_EXTRA[lc as usize], "extra {extra} for len {len}");
        assert!(lc < 29);
    }
}

#[test]
fn distance_code_exhaustive_roundtrip() {
    for dist in 1..=32_768u32 {
        let (dc, extra) = distance_code(dist);
        assert_eq!(DIST_BASE[dc as usize] + extra, dist, "dist {dist}");
        assert!(extra < 1 << DIST_EXTRA[dc as usize], "extra {extra} for dist {dist}");
        assert!(dc < 30);
    }
}

#[test]
fn code_picker_boundaries() {
    // Length boundaries: 3 (code 257, 0 extra) .. 258 (code 285, 0 extra).
    assert_eq!(length_code(3), (0, 0));
    assert_eq!(length_code(10), (7, 0));
    assert_eq!(length_code(11), (8, 0));
    assert_eq!(length_code(12), (8, 1));
    assert_eq!(length_code(257), (27, 30)); // 227 + 30
    assert_eq!(LEN_BASE[27] + 30, 257);
    assert_eq!(length_code(258), (28, 0));
    // Distance boundaries: 1, 32768, and the 24577..32768 band (code 29).
    assert_eq!(distance_code(1), (0, 0));
    assert_eq!(distance_code(2), (1, 0));
    assert_eq!(distance_code(32_768), (29, 8_191));
    assert_eq!(distance_code(24_577), (29, 0));
    assert_eq!(distance_code(24_577 + 8_190), (29, 8_190));
    assert_eq!(distance_code(16_385), (28, 0));
    assert_eq!(distance_code(16_384), (27, 4_095));
}

#[test]
fn code_table_decodes_fixed_lit() {
    let t = CodeTable::build(&FIXED_LIT_LENGTHS);
    let codes = canonical_codes(&FIXED_LIT_LENGTHS, 9);
    for sym in 0..288usize {
        let len = FIXED_LIT_LENGTHS[sym] as u32;
        let got = t.decode(codes[sym], len);
        assert_eq!(got, Some(sym), "symbol {sym}");
    }
    // An unknown (code, len) pair decodes to None.
    assert_eq!(t.decode(0, 15), None); // length 15 not in the table
}

#[test]
fn code_table_overfull_panics() {
    // 5 codes of length 2: Kraft 5/4 > 1 — overfull must panic (caught).
    let r = std::panic::catch_unwind(|| {
        CodeTable::build(&[2, 2, 2, 2, 2]);
    });
    assert!(r.is_err(), "overfull table must panic");
}

#[test]
fn code_table_underfull_is_legal() {
    // 2 codes of length 2: Kraft 1/2 — underfull but decodable (RFC-legal).
    let t = CodeTable::build(&[2, 2, 0]);
    assert!(t.max_length() >= 1);
}

#[test]
fn code_length_order_pinned() {
    assert_eq!(
        CODE_LENGTH_ORDER,
        [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
    );
}

#[test]
fn run_coding_16() {
    let (syms, reps) = encode_code_lengths(&[8u8; 12]);
    // RFC §3.2.7 example: 8, 16(+11b), 16(+10b) -> 1 + 6 + 5 = 12.
    assert_eq!(syms, vec![8, 16, 16]);
    assert_eq!(reps, vec![0, 3, 2]);
}

#[test]
fn run_coding_17_18() {
    let mut seq: Vec<u8> = vec![5];
    seq.extend(std::iter::repeat(0).take(9));
    seq.push(7);
    seq.extend(std::iter::repeat(0).take(20));
    let (syms, reps) = encode_code_lengths(&seq);
    assert_eq!(syms, vec![5, 17, 7, 18]);
    assert_eq!(reps, vec![0, 6, 0, 9]);

    // 200 zeros: 18(138) + 18(62).
    let (syms, reps) = encode_code_lengths(&vec![0u8; 200]);
    assert_eq!(syms, vec![18, 18]);
    assert_eq!(reps, vec![127, 51]);

    // Short zero runs stay literal.
    let (syms, _) = encode_code_lengths(&[1, 0, 0, 2]);
    assert_eq!(syms, vec![1, 0, 0, 2]);
}

/// Decode via indexed callbacks (independent counters per vector).
fn decode_seq(seq: &[u8]) -> Option<Vec<u8>> {
    let (syms, reps) = encode_code_lengths(seq);
    let rep_vals: Vec<u32> = syms
        .iter()
        .zip(reps.iter())
        .filter(|(s, _)| **s >= 16)
        .map(|(_, r)| *r)
        .collect();
    struct St {
        syms: Vec<u32>,
        rep_vals: Vec<u32>,
        i_sym: usize,
        i_rep: usize,
    }
    let st = std::rc::Rc::new(std::cell::RefCell::new(St {
        syms,
        rep_vals,
        i_sym: 0,
        i_rep: 0,
    }));
    let st1 = st.clone();
    let st2 = st.clone();
    decode_code_lengths(
        || {
            let mut g = st1.borrow_mut();
            if g.i_sym >= g.syms.len() {
                return Err(BitError::Exhausted);
            }
            let s = g.syms[g.i_sym];
            g.i_sym += 1;
            Ok(s)
        },
        |_| {
            let mut g = st2.borrow_mut();
            if g.i_rep >= g.rep_vals.len() {
                return Err(BitError::Exhausted);
            }
            let r = g.rep_vals[g.i_rep];
            g.i_rep += 1;
            Ok(r as u64)
        },
        seq.len(),
    )
    .ok()
}

#[test]
fn encode_decode_roundtrip_many() {
    let mut seqs: Vec<Vec<u8>> = Vec::new();
    seqs.push(vec![8; 12]);
    let mut s: Vec<u8> = vec![9];
    s.extend(std::iter::repeat(9).take(29));
    seqs.push(s);
    seqs.push(vec![0; 199]);
    seqs.push(vec![3, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5, 6]);
    // A realistic 288+30 profile: many 0s, 3..9 lengths.
    let mut p: Vec<u8> = (0..288).map(|i| if i % 7 == 0 { 3 + (i % 7) as u8 } else { 0 }).collect();
    p.extend(std::iter::repeat(5).take(30));
    seqs.push(p);
    for (i, seq) in seqs.iter().enumerate() {
        assert_eq!(decode_seq(seq).as_deref(), Some(seq.as_slice()), "seq {i}");
    }
}

#[test]
fn decode_rejects_16_at_sequence_start() {
    // A 16 with nothing before it must be an error.
    let syms = vec![16u32, 16];
    let reps = vec![0u32, 3];
    let rep_vals: Vec<u32> = syms.iter().zip(reps.iter()).map(|(_, r)| *r).collect();
    struct St {
        syms: Vec<u32>,
        rep_vals: Vec<u32>,
        i_sym: usize,
        i_rep: usize,
    }
    let st = std::rc::Rc::new(std::cell::RefCell::new(St {
        syms,
        rep_vals,
        i_sym: 0,
        i_rep: 0,
    }));
    let st1 = st.clone();
    let st2 = st.clone();
    let r = decode_code_lengths(
        || {
            let mut g = st1.borrow_mut();
            if g.i_sym >= g.syms.len() {
                return Err(BitError::Exhausted);
            }
            let s = g.syms[g.i_sym];
            g.i_sym += 1;
            Ok(s)
        },
        |_| {
            let mut g = st2.borrow_mut();
            if g.i_rep >= g.rep_vals.len() {
                return Err(BitError::Exhausted);
            }
            let r = g.rep_vals[g.i_rep];
            g.i_rep += 1;
            Ok(r as u64)
        },
        4,
    );
    assert!(r.is_err());
}
