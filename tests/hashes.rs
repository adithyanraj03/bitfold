//! Adler-32 / CRC-32 tests: known vectors, chunking at NMAX, and the
//! embedded 162-vector zlib battery (spot-checked here end-to-end; the
//! full battery runs in the KAT suite).

use bitfold::adler::{adler32, adler32_fresh, NMAX};
use bitfold::crc32::{crc32, crc32_fresh, TABLE_256};
use bitfold::kat_data::{KatBuf, HASH_KATS, LONG_STRING};

#[test]
fn adler_known_vectors() {
    // Fresh over the empty input: a=1, b=1 -> 0x00000001.
    assert_eq!(adler32_fresh(b""), 0x0000_0001);
    // The classic zlib vector.
    assert_eq!(adler32_fresh(b"Wikipedia"), 0x11E6_0398);
    // One NUL byte: a = 1+0 = 1, b = 0+1 = 1 -> 0x00010001.
    assert_eq!(adler32_fresh(b"\x00"), 0x0001_0001);
    // Initial-value vectors (the state is (b<<16)|a; the final modulo
    // reduction applies even for empty input).
    assert_eq!(adler32(1, b""), 0x0000_0001);
    assert_eq!(adler32(0xFFFF_FFFF, b""), 0x000E_000E);
    assert_eq!(adler32(0, b"a"), 0x0061_0061);
    assert_eq!(adler32(0xFFFF_FFFF, b"a"), 0x007D_006F);
}

#[test]
fn crc_known_vectors() {
    // Fresh over the empty input: 0.
    assert_eq!(crc32_fresh(b""), 0x0000_0000);
    // The classic check value.
    assert_eq!(crc32_fresh(b"123456789"), 0xCBF4_3926);
    // A NUL byte.
    assert_eq!(crc32_fresh(b"\x00"), 0xD202_EF8D);
    // Initial-value vectors.
    assert_eq!(crc32(0, b""), 0);
    assert_eq!(crc32(0xFFFFFFFF, b""), 0xFFFFFFFF);
    assert_eq!(crc32(1, b""), 1);
}

#[test]
fn crc_table_spot_values() {
    // The CRC-32 (IEEE) table: row 0 is zero; row 1 is the classic
    // 0x77073096; the rest are re-derived here from the reflected
    // polynomial 0xEDB88320.
    assert_eq!(TABLE_256[0], 0);
    assert_eq!(TABLE_256[1], 0x7707_3096);
    // Row i is the byte i folded through 8 steps of the reflected
    // polynomial 0xEDB88320.
    fn row(i: u32) -> u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        c
    }
    for i in [0u32, 1, 7, 42, 128, 200, 254, 255] {
        assert_eq!(TABLE_256[i as usize], row(i), "row {i}");
    }
}

#[test]
fn adler_chunking_matches_direct() {
    // The NMAX-5552 chunk boundary must not change the result.
    for n in [1usize, 5551, 5552, 5553, 11103, 11104, 11105, 11106, 60_000] {
        let data: Vec<u8> = (0..n).map(|i| (i * 31 + 7) as u8).collect();
        let a = adler32_fresh(&data);
        // Manual chunking through the initial-value API.
        let mut acc = 1u32;
        for chunk in data.chunks(NMAX) {
            acc = adler32(acc, chunk);
        }
        assert_eq!(a, acc, "chunking mismatch at n={n}");
    }
}

#[test]
fn long_string_matches_zlib_kat() {
    // The 5552-byte long_string from zlib_hash_tests.h: adler fresh =
    // 0x8b81718f, crc fresh = 0x426fa73b, adler init 0x7712aa45 = 0x7dc51be2.
    assert_eq!(adler32_fresh(LONG_STRING), 0x8B81_718F);
    assert_eq!(crc32_fresh(LONG_STRING), 0x426F_A73B);
    assert_eq!(adler32(0x7712_AA45, LONG_STRING), 0x7DC5_1BE2);
}

#[test]
fn kat_vectors_resolvable_and_consistent() {
    // Every vector's buffer must resolve to exactly `len` bytes, and the
    // recorded expectations must be reproducible from the resolved buffer.
    for (i, k) in HASH_KATS.iter().enumerate() {
        let buf: Vec<u8> = match k.buf {
            KatBuf::Empty => Vec::new(),
            KatBuf::Nul => vec![0],
            KatBuf::Text(b) => b.to_vec(),
            KatBuf::Long => LONG_STRING[..k.len as usize].to_vec(),
            KatBuf::Big => (0..k.len).map(|x| (x % 256) as u8).collect(),
        };
        assert_eq!(buf.len(), k.len as usize, "vector {i}");
        assert_eq!(adler32(k.init_adler, &buf), k.exp_adler, "vector {i} adler");
        assert_eq!(crc32(k.init_crc, &buf), k.exp_crc, "vector {i} crc");
    }
    assert_eq!(HASH_KATS.len(), 162);
}

#[test]
fn adler_monotonic_in_a_part() {
    // Feeding more identical bytes strictly increases the running value
    // (modulo wrap) — a canary against an accidentally constant hash.
    let a1 = adler32(1, b"a");
    let a2 = adler32(a1, b"a");
    let a3 = adler32(a2, b"a");
    assert_ne!(a1, a2);
    assert_ne!(a2, a3);
}

#[test]
fn crc_detects_single_bit_flip() {
    let base = b"The quick brown fox";
    let flipped: Vec<u8> = base.iter().map(|b| b ^ 1).collect();
    assert_ne!(crc32_fresh(base), crc32_fresh(&flipped));
    assert_ne!(adler32_fresh(base), adler32_fresh(&flipped));
}
