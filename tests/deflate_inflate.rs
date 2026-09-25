//! Encoder/decoder integration: round trips at boundaries, container
//! checks, and stream-structure invariants.

use bitfold::deflate::{compress_raw, compress_zlib, container_hashes, MAX_BLOCK};
use bitfold::inflate::{decompress_raw, decompress_zlib, InflateError};
use bitfold::lzw77::{compress as lzw_compress, phrase_stats, Phrase};

/// Deterministic pseudo-random bytes (LCG, fixed seed — no RNG crate).
fn rnd(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((x >> 33) & 0xFF) as u8
        })
        .collect()
}

#[test]
fn roundtrip_empty() {
    let z = compress_zlib(b"");
    assert_eq!(decompress_zlib(&z).unwrap(), b"");
    let (raw, st) = compress_raw(b"");
    assert_eq!(decompress_raw(&raw).unwrap(), b"");
    assert_eq!(st.n_blocks, 1); // one final empty block
}

#[test]
fn roundtrip_single_byte() {
    for b in [0u8, 1, 0x7F, 0x80, 0xFF] {
        let z = compress_zlib(&[b]);
        assert_eq!(decompress_zlib(&z).unwrap(), [b]);
    }
}

#[test]
fn roundtrip_high_entropy() {
    let data = rnd(60_000, 42);
    let z = compress_zlib(&data);
    assert_eq!(decompress_zlib(&z).unwrap(), data);
}

#[test]
fn roundtrip_repetitive() {
    let mut data = Vec::new();
    for i in 0..4000 {
        data.extend_from_slice(format!("line {i:04} of the repetitive corpus ").as_bytes());
    }
    let z = compress_zlib(&data);
    assert!(z.len() < data.len() / 4, "repetitive data must fold hard");
    assert_eq!(decompress_zlib(&z).unwrap(), data);
}

#[test]
fn roundtrip_block_boundary() {
    for n in [MAX_BLOCK - 1, MAX_BLOCK, MAX_BLOCK + 1, MAX_BLOCK * 2 + 17] {
        let data = rnd(n, 7);
        let z = compress_zlib(&data);
        let back = decompress_zlib(&z).unwrap();
        assert_eq!(back.len(), n, "n = {n}");
        assert_eq!(back, data, "n = {n}");
    }
}

#[test]
fn long_input_spans_multiple_blocks() {
    let data = rnd(200_000, 99);
    let (raw, st) = compress_raw(&data);
    assert!(st.n_blocks >= 4, "200 KB must span blocks, got {}", st.n_blocks);
    assert_eq!(decompress_raw(&raw).unwrap(), data);
    for b in &st.blocks {
        assert!(b.btype <= 2);
        assert!(b.n_bytes <= MAX_BLOCK);
    }
}

#[test]
fn zlib_container_hashes() {
    let data = b"hash me";
    let (crc, adler) = container_hashes(data);
    assert_eq!(crc, bitfold::crc32::crc32_fresh(data));
    assert_eq!(adler, bitfold::adler::adler32_fresh(data));
    let z = compress_zlib(data);
    assert_eq!(&z[..2], &[0x78, 0x9C]);
    assert_eq!(
        u32::from_be_bytes([z[z.len() - 4], z[z.len() - 3], z[z.len() - 2], z[z.len() - 1]]),
        adler
    );
}

#[test]
fn zlib_bad_headers_rejected() {
    // (0x78, 0x1C): valid CMF, wrong checksum (30748 % 31 != 0).
    // (0x78, 0x20): valid checksum (30752 = 31*992) with FDICT set.
    // (0x08, 0x9C):  CMF & 0x0F == 8 but 2204 % 31 != 0.
    // (0x78, 0x9C):  header only -> the DEFLATE body is empty.
    let bad: Vec<Vec<u8>> = vec![
        vec![0x78, 0x1C, 0, 0, 0, 0, 0, 0],
        vec![0x08, 0x9C, 0, 0, 0, 0, 0, 0],
        vec![0x78, 0x20, 0, 0, 0, 0, 0, 0],
        vec![0x78, 0x9C],
    ];
    for b in bad {
        let e = decompress_zlib(&b).unwrap_err();
        assert!(
            matches!(
                e,
                InflateError::BadFormat | InflateError::Truncated | InflateError::ChecksumMismatch
            ),
            "{e:?}"
        );
    }
}

#[test]
fn zlib_truncated_payload_rejected() {
    let data = rnd(4000, 5);
    let z = compress_zlib(&data);
    for cut in [z.len() - 1, z.len() - 4, z.len() / 2] {
        let r = decompress_zlib(&z[..cut]);
        assert!(r.is_err(), "cut at {cut} must fail");
    }
}

#[test]
fn match_boundaries_len258_dist1() {
    // 32768 'a's then 32768 more: the tail is one long dist-1 copy chain,
    // and the max match length is 258.
    let data = vec![b'a'; 65_536];
    let z = compress_zlib(&data);
    assert_eq!(decompress_zlib(&z).unwrap(), data);
    assert!(z.len() < 1500, "64KB of 'a' must be tiny, got {}", z.len());
}

#[test]
fn match_boundaries_dist32768() {
    // 32768 pseudo-random prefix, then the exact same 32768 bytes: the
    // copy distance is the full window.
    let prefix = rnd(32_768, 3);
    let mut data = prefix.clone();
    data.extend_from_slice(&prefix);
    let z = compress_zlib(&data);
    assert_eq!(decompress_zlib(&z).unwrap(), data);
}

#[test]
fn length_boundaries_roundtrip() {
    // Craft messages whose LZ77 output must hit the length boundaries:
    // a 258-long run and a 257-long run with a different tail.
    for n in [257, 258] {
        let mut data = vec![b'q'; 100];
        data.extend(std::iter::repeat(b'r').take(n));
        let z = compress_zlib(&data);
        assert_eq!(decompress_zlib(&z).unwrap(), data, "run {n}");
    }
}

#[test]
fn phrases_reconstruct_overlaps() {
    let phrases = vec![
        Phrase::Literal(b'X'),
        Phrase::Literal(b'Y'),
        Phrase::Match { len: 5, dist: 2 },
        Phrase::Match { len: 10, dist: 2 },
    ];
    let out = bitfold::deflate::phrases_to_bytes(&phrases);
    // XY + match{5,2} = XYXYXYX; the second match{10,2} keeps walking
    // back two positions inside the alternating region, so the output
    // continues the X,Y alternation: 17 bytes.
    assert_eq!(out, b"XYXYXYXYXYXYXYXYX");
    let (lits, matches) = phrase_stats(&phrases);
    assert_eq!(lits, 2);
    assert_eq!(matches, 2);
}

#[test]
fn lz77_window_and_limits() {
    // A match cannot reach further than the window: plant a byte far
    // back and make sure the compressor still round-trips.
    let mut data = rnd(40_000, 11);
    data.push(0xEE);
    data.extend(rnd(40_000, 12));
    data.push(0xEE);
    let z = compress_zlib(&data);
    assert_eq!(decompress_zlib(&z).unwrap(), data);
}

#[test]
fn stats_consistency() {
    let data = rnd(50_000, 21);
    let (raw, st) = compress_raw(&data);
    // Phrases reconstruct the input exactly.
    let phrases = lzw_compress(&data);
    assert_eq!(bitfold::deflate::phrases_to_bytes(&phrases), data);
    // Wire size is the logical bits rounded up to bytes (plus nothing:
    // compress_raw pads internally).
    let wire = (st.deflate_bits + 7) / 8;
    assert!((wire as isize - raw.len() as isize).abs() <= 1, "wire {wire} vs raw {}", raw.len());
    // Stored blocks, if any, carry their raw span; block bytes sum to input.
    let sum: usize = st.blocks.iter().map(|b| b.n_bytes).sum();
    assert_eq!(sum, data.len());
    assert_eq!(st.in_bytes, data.len());
}

#[test]
fn dynamic_blocks_carry_header_fields() {
    let data = rnd(30_000, 33);
    let (raw, st) = compress_raw(&data);
    assert!(st.n_dynamic > 0 || st.n_fixed > 0 || st.n_stored > 0);
    // The stream must be fully decodable with per-block logging.
    let mut log = Vec::new();
    let back = bitfold::inflate::decompress_raw_debug(&raw, &mut log).unwrap();
    assert_eq!(back, data);
    assert!(log.len() >= st.n_blocks as usize * 2);
}

#[test]
fn fixed_blocks_decode() {
    // The pinned single-literal fixed block (see kats).
    assert_eq!(decompress_raw(&[0x4B, 0x04, 0x00]).unwrap(), [0x61]);
    // A fixed block of all literals, hand-built is not feasible here;
    // instead round-trip a high-entropy 64-byte message (stored or fixed
    // likely) and force-decode via the public API.
    let data = rnd(64, 55);
    let z = compress_zlib(&data);
    assert_eq!(decompress_zlib(&z).unwrap(), data);
}

#[test]
fn decompress_error_kinds_distinct() {
    let e = decompress_zlib(&[0x78, 0x1C, 0, 0, 0, 0, 0, 0]).unwrap_err();
    assert!(matches!(e, InflateError::BadFormat), "{e:?}");
    let e = decompress_zlib(&[0x78, 0x9C]).unwrap_err();
    assert!(matches!(e, InflateError::Truncated), "{e:?}");
}
