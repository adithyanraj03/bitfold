//! Known-answer test battery.
//!
//! Five groups, in order:
//!
//! * **rfc1951** — the RFC's own worked examples and tables: the §3.2.2
//!   canonical-assignment example, the §3.2.6 fixed-code spot values, the
//!   §3.2.5 length/distance base+extra tables, the §3.2.7 19-symbol
//!   code-length order, the §3.2.7 run example (twelve 8s), exhaustive
//!   `length_code` / `distance_code` round-trips, and 16/17/18 run
//!   round-trips through the public encoder/decoder.
//! * **bits** — pinned bit-packing streams pinning the two element orders
//!   (LSB-first data vs MSB-first Huffman codes) and the 64-bit boundary.
//! * **blocks** — pinned single-block raw DEFLATE streams (stored and
//!   fixed), the overlapping-match semantics, the zlib container for a
//!   pinned message, and malformed-container rejection.
//! * **hash** — the 162-vector `hash_tests[]` table from zlib's
//!   `zlib_hash_tests.h`, embedded verbatim (see `kat_data` for the
//!   extraction provenance) and verified here against our `adler32` /
//!   `crc32`.
//! * **xval** — cross-verification against CPython's `zlib` (performed at
//!   development time; the pinned hex below is the contract): our
//!   encoder's dynamic stream must be byte-identical on every run, and
//!   CPython's fixed-block streams must decode byte-identically here.
//!
//! Every check is deterministic and pure — `run_all()` may be called any
//! number of times with the same result.

use std::cell::RefCell;
use std::rc::Rc;

use crate::adler::adler32;
use crate::bits::{BitError, BitReader, BitWriter};
use crate::crc32::crc32;
use crate::deflate::{compress_zlib, container_hashes};
use crate::inflate::{decompress_raw, decompress_zlib, InflateError};
use crate::kat_data::{KatBuf, HASH_KATS, LONG_STRING};
use crate::lzw77::Phrase;
use crate::deflate::phrases_to_bytes;

/// One battery check.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// Run the full battery.
pub fn run_all() -> Vec<Check> {
    let mut v = Vec::new();
    v.extend(rfc_kats());
    v.extend(bit_kats());
    v.extend(block_kats());
    v.extend(hash_battery());
    v.extend(cross_kats());
    v
}

/// The number of checks in the full battery (without running it).
#[must_use]
pub fn check_count() -> usize {
    13 + 4 + 5 + HASH_KATS.len() + 4
}

fn chk(name: &str, ok: bool, detail: String) -> Check {
    Check {
        name: name.to_string(),
        ok,
        detail,
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The 2250-byte cross-verification message (50 × 45).
fn fox_msg() -> Vec<u8> {
    let mut v = Vec::with_capacity(2250);
    for _ in 0..50 {
        v.extend_from_slice(b"The quick brown fox jumps over the lazy dog. ");
    }
    v
}

/// The 3000-byte incompressible cross-verification message
/// (`(i * 2654435761) mod 256`, the Knuth multiplicative hash).
fn data_msg() -> Vec<u8> {
    // u64 arithmetic: `i * 2654435761` overflows u32 for i >= 2 (Python's
    // arbitrary precision made this invisible at pinning time).
    (0..3000u32).map(|i| ((i as u64 * 2_654_435_761) % 256) as u8).collect()
}

// ------------------------------------------------------------------
// RFC 1951 group
// ------------------------------------------------------------------

fn rfc_kats() -> Vec<Check> {
    use crate::huff::{
        canonical_codes, distance_code, encode_code_lengths, length_code,
        CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, FIXED_DIST_LENGTHS,
        FIXED_LIT_LENGTHS, LEN_BASE, LEN_EXTRA,
    };
    let mut v = Vec::new();

    // §3.2.2 worked example: lengths (3,3,3,3,3,2,4,4) for A..H.
    let codes = canonical_codes(&[3, 3, 3, 3, 3, 2, 4, 4], 4);
    v.push(chk(
        "rfc-canonical-example",
        codes == [2, 3, 4, 5, 6, 0, 14, 15],
        format!("A..H codes = {codes:?} (want A=010 B=011 C=100 D=101 E=110 F=00 G=1110 H=1111)"),
    ));

    // §3.2.6 fixed literal/length spot codes.
    let fl = canonical_codes(&FIXED_LIT_LENGTHS, 9);
    let spot = [
        (0u32, 0b00110000),
        (143, 0b10111111),
        (144, 0b110010000),
        (255, 0b111111111),
        (256, 0b0000000),
        (279, 0b0010111),
        (280, 0b11000000),
        (285, 0b11000101),
        (287, 0b11000111),
    ];
    let ok = spot.iter().all(|&(sym, c)| fl[sym as usize] == c);
    v.push(chk(
        "rfc-fixed-lit-codes",
        ok,
        format!("lit0={} lit143={} lit144={} lit256={} lit285={}", fl[0], fl[143], fl[144], fl[256], fl[285]),
    ));

    // §3.2.6 fixed distance codes: 5-bit identity codes.
    let fd = canonical_codes(&FIXED_DIST_LENGTHS, 5);
    let ok = fd.iter().zip(0..30u32).all(|(&c, i)| c == i);
    v.push(chk(
        "rfc-fixed-dist-codes",
        ok,
        format!("dist codes = identity over 0..29 (first 8: {:?})", &fd[..8]),
    ));

    v.push(chk(
        "rfc-len-base-table",
        LEN_BASE == [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99,
            115, 131, 163, 195, 227, 258,
        ],
        format!("29 length bases, 3..258 ({})", LEN_BASE.len()),
    ));
    v.push(chk(
        "rfc-len-extra-table",
        LEN_EXTRA == [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
        ],
        format!("29 length extra counts (codes 277-280 use 4, 281-284 use 5)"),
    ));
    v.push(chk(
        "rfc-dist-base-table",
        DIST_BASE == [
            1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025,
            1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
        ],
        format!("30 distance bases, 1..32768 ({})", DIST_BASE.len()),
    ));
    v.push(chk(
        "rfc-dist-extra-table",
        DIST_EXTRA == [
            0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
            12, 13, 13,
        ],
        "30 distance extra counts (codes 28-29 use 13)".to_string(),
    ));
    v.push(chk(
        "rfc-code-length-order",
        CODE_LENGTH_ORDER
            == [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15],
        format!("19-symbol on-the-wire order = {CODE_LENGTH_ORDER:?}"),
    ));

    // Exhaustive length-code round-trip (3..=258).
    let mut bad = Vec::new();
    for len in 3..=258u32 {
        let (lc, extra) = length_code(len);
        if LEN_BASE[lc as usize] + extra != len || extra >= 1 << LEN_EXTRA[lc as usize] {
            bad.push(len);
        }
    }
    v.push(chk(
        "rfc-length-code-roundtrip",
        bad.is_empty(),
        format!("256 lengths, first bad = {}", bad.first().map_or("none".to_string(), |x| x.to_string())),
    ));

    // Exhaustive distance-code round-trip (1..=32768).
    let mut bad = Vec::new();
    for dist in 1..=32768u32 {
        let (dc, extra) = distance_code(dist);
        if DIST_BASE[dc as usize] + extra != dist || extra >= 1 << DIST_EXTRA[dc as usize] {
            bad.push(dist);
        }
    }
    v.push(chk(
        "rfc-distance-code-roundtrip",
        bad.is_empty(),
        format!("32768 distances, first bad = {}", bad.first().map_or("none".to_string(), |x| x.to_string())),
    ));

    // §3.2.7 run example: twelve 8s -> literal, then two 16-runs.
    let (syms, reps) = encode_code_lengths(&[8u8; 12]);
    v.push(chk(
        "rfc-run-example-8x12",
        syms == vec![8, 16, 16] && reps == vec![0, 3, 2],
        format!("syms = {syms:?} reps = {reps:?}"),
    ));

    // 17/18 runs: 9 zeros -> one 17; 20 zeros -> one 18.
    let mut seq: Vec<u8> = vec![5];
    seq.extend(std::iter::repeat(0).take(9));
    seq.push(7);
    seq.extend(std::iter::repeat(0).take(20));
    let (syms, reps) = encode_code_lengths(&seq);
    v.push(chk(
        "rfc-repeat-17-18-runs",
        syms == vec![5, 17, 7, 18] && reps == vec![0, 6, 0, 9],
        format!("syms = {syms:?} reps = {reps:?}"),
    ));

    // Encode/decode round-trips through the public generic decoder.
    let seqs: Vec<Vec<u8>> = {
        let mut a: Vec<u8> = vec![8];
        a.extend(std::iter::repeat(8).take(11));
        let mut b: Vec<u8> = vec![9];
        b.extend(std::iter::repeat(9).take(29));
        let mut c: Vec<u8> = vec![0];
        c.extend(std::iter::repeat(0).take(199));
        let d = vec![3, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5, 6];
        vec![a, b, c, d]
    };
    let mut all_ok = true;
    let mut first_fail = String::new();
    for (i, seq) in seqs.iter().enumerate() {
        let got = decode_seq(seq);
        if got.as_deref() != Some(seq.as_slice()) {
            all_ok = false;
            first_fail = format!("seq {i} (len {})", seq.len());
            break;
        }
    }
    v.push(chk(
        "run-encode-decode-roundtrip",
        all_ok,
        format!("4 synthetic sequences ({}..{} symbols)", first_fail, "ok"),
    ));

    v
}

/// Decode a code-length sequence with the public generic decoder, using
/// indexed callbacks over the encoder's (symbols, reps) output. Symbols
/// and repeat values advance through **independent** counters: `read_bits`
/// is only ever called for the 16/17/18 symbols, so it serves the
/// pre-filtered repeat values in the order the repeats appear.
fn decode_seq(seq: &[u8]) -> Option<Vec<u8>> {
    use crate::huff::{decode_code_lengths, encode_code_lengths};
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
    let st = Rc::new(RefCell::new(St {
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
        |_n| {
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

// ------------------------------------------------------------------
// Bit-packing group
// ------------------------------------------------------------------

fn bit_kats() -> Vec<Check> {
    let mut v = Vec::new();

    // LSB-first pattern: 0b110 (3 bits) + 0b1010 (4 bits) + 1 (1 bit).
    // Stream bits: 0,1,1 | 0,1,0,1 | 1  -> byte 0b01101011 reversed = 0xD6.
    let mut w = BitWriter::new();
    w.push_lsb(0b110, 3);
    w.push_lsb(0b1010, 4);
    w.push_lsb(1, 1);
    let out = w.finish();
    let ok = out == [0xD6];
    v.push(chk(
        "bit-lsb-pattern",
        ok,
        format!("lsb(0b110)+lsb(0b1010)+lsb(1) = {} (want 0xd6)", to_hex(&out)),
    ));

    // Same values, MSB-first (Huffman) order: bits 1,1,0 | 1,0,1,0 | 1
    // -> LSB-first byte 0b01011011 = 0xAB (bits reversed within the byte
    // relative to the code order).
    let mut w = BitWriter::new();
    w.push_huff(0b110, 3);
    w.push_huff(0b1010, 4);
    w.push_huff(1, 1);
    let out = w.finish();
    let ok = out == [0xAB];
    v.push(chk(
        "bit-huff-pattern",
        ok,
        format!("huff(0b110)+huff(0b1010)+huff(1) = {} (want 0xab)", to_hex(&out)),
    ));

    // 64-bit LSB element: byte order must be little-endian.
    let val = 0xDEAD_BEEF_CAFE_1234u64;
    let mut w = BitWriter::new();
    w.push_lsb(val, 64);
    let out = w.finish();
    let want: [u8; 8] = [0x34, 0x12, 0xFE, 0xCA, 0xEF, 0xBE, 0xAD, 0xDE];
    let mut r = BitReader::new(&out);
    let back = r.next_lsb(64).unwrap_or(u64::MAX);
    v.push(chk(
        "bit-64-lsb",
        out == want && back == val,
        format!("lsb64(0xdeadbeefcafe1234) = {}, read back = 0x{back:016x}", to_hex(&out)),
    ));

    // 64-bit MSB element: each output byte is the 8 code bits with the
    // bit order reversed within the byte (LSB-first packing of MSB-first
    // codes) — pinned against an independent Python computation.
    let mut w = BitWriter::new();
    w.push_huff((val >> 32) as u32, 32);
    w.push_huff((val & 0xFFFF_FFFF) as u32, 32);
    let out = w.finish();
    let want: [u8; 8] = [0x7B, 0xB5, 0x7D, 0xF7, 0x53, 0x7F, 0x48, 0x2C];
    let mut r = BitReader::new(&out);
    let hi = r.next_huff(32).unwrap_or(u32::MAX) as u64;
    let lo = r.next_huff(32).unwrap_or(u32::MAX) as u64;
    let back = (hi << 32) | lo;
    v.push(chk(
        "bit-64-huff",
        out == want && back == val,
        format!("huff64(0xdeadbeefcafe1234) = {}, read back = 0x{back:016x}", to_hex(&out)),
    ));

    v
}

// ------------------------------------------------------------------
// Block / container group
// ------------------------------------------------------------------

fn block_kats() -> Vec<Check> {
    use crate::crc32::crc32_fresh;
    use crate::adler::adler32_fresh;
    let mut v = Vec::new();

    // Pinned stored block: bfinal=1, btype=00, LEN=3, NLEN=0xFFFC, "ABC".
    let stored = [0x01, 0x03, 0x00, 0xFC, 0xFF, 0x41, 0x42, 0x43];
    let back = decompress_raw(&stored).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "stored-block-pinned",
        back.as_deref() == Ok(b"ABC"),
        format!("raw stored 0x{} -> {back:?}", to_hex(&stored)),
    ));

    // Pinned fixed block: bfinal=1, btype=01, literal 'a' (code 145, 8
    // bits), EOB (0, 7 bits) -> 18 bits, 3 bytes.
    let fixed = [0x4B, 0x04, 0x00];
    let back = decompress_raw(&fixed).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "fixed-block-pinned",
        back.as_deref() == Ok([0x61].as_slice()),
        format!("raw fixed 0x{} -> {back:?}", to_hex(&fixed)),
    ));

    // Overlapping match: X, Y, then match{len 5, dist 2} -> XYXYY...
    // copy: X Y X Y X  =>  "XYXYXYX".
    let phrases = vec![
        Phrase::Literal(b'X'),
        Phrase::Literal(b'Y'),
        Phrase::Match { len: 5, dist: 2 },
    ];
    let out = phrases_to_bytes(&phrases);
    v.push(chk(
        "overlap-match-kat",
        out == *b"XYXYXYX",
        format!("XY + match{{5,2}} -> {:?}", String::from_utf8_lossy(&out)),
    ));

    // Pinned zlib container for "hello": 0x78 0x9C + 5 deflate bytes +
    // Adler. Container hashes pinned to the known CRC-32 / Adler-32.
    let hello = b"hello";
    let z = compress_zlib(hello);
    let want = hex_bytes("789ccb48cdc9c90700062c0215");
    let back = decompress_zlib(&z).map_err(|e| format!("{e:?}"));
    let (crc, adler) = container_hashes(hello);
    let hashes_ok = crc == crc32_fresh(hello) && adler == adler32_fresh(hello);
    v.push(chk(
        "zlib-container-hello",
        z == want && back.as_deref() == Ok(hello) && hashes_ok,
        format!(
            "z(5B) = {} (pinned), crc32 = 0x{crc:08x}, adler32 = 0x{adler:08x}",
            to_hex(&z)
        ),
    ));

    // Malformed containers must be rejected (not panic, not misdecode).
    let bad: Vec<(&str, Vec<u8>)> = vec![
        ("bad-checksum", vec![0x78, 0x01, 0x00, 0x00, 0x00, 0x00]),
        ("bad-cmf", vec![0x08, 0x9C, 0x00, 0x00, 0x00, 0x00]),
        ("fdict-rejected", vec![0x78, 0x20, 0x00, 0x00, 0x00, 0x00]),
        ("truncated", hex_bytes("789c").to_vec()),
    ];
    let mut all_rejected = true;
    let mut detail = String::new();
    for (name, bytes) in &bad {
        let r = decompress_zlib(bytes);
        let rejected = matches!(r, Err(InflateError::BadFormat | InflateError::Truncated | InflateError::ChecksumMismatch));
        detail.push_str(&format!("{name}={:?} ", r));
        if !rejected {
            all_rejected = false;
        }
    }
    v.push(chk("zlib-headers-rejected", all_rejected, detail));

    v
}

// ------------------------------------------------------------------
// zlib hash battery (162 vectors)
// ------------------------------------------------------------------

fn buf_for(k: &crate::kat_data::Kat) -> Vec<u8> {
    match k.buf {
        KatBuf::Empty => Vec::new(),
        KatBuf::Nul => vec![0],
        KatBuf::Text(b) => b.to_vec(),
        KatBuf::Long => LONG_STRING[..k.len as usize].to_vec(),
        KatBuf::Big => (0..k.len).map(|i| (i % 256) as u8).collect(),
    }
}

fn hash_battery() -> Vec<Check> {
    HASH_KATS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let buf = buf_for(k);
            let got_a = adler32(k.init_adler, &buf);
            let got_c = crc32(k.init_crc, &buf);
            let ok = got_a == k.exp_adler && got_c == k.exp_crc;
            chk(
                &format!("hash-{:03}", i + 1),
                ok,
                format!(
                    "len {} adler(0x{:08x})=0x{:08x} crc(0x{:08x})=0x{:08x}",
                    k.len, k.init_adler, got_a, k.init_crc, got_c
                ),
            )
        })
        .collect()
}

// ------------------------------------------------------------------
// Cross-verification group (CPython zlib, pinned at dev time)
// ------------------------------------------------------------------

fn cross_kats() -> Vec<Check> {
    let mut v = Vec::new();
    let fox = fox_msg();
    let data = data_msg();

    // Our encoder's dynamic stream for the fox message: pinned for
    // determinism and verified byte-identical by CPython zlib.decompress
    // at development time.
    let z = compress_zlib(&fox);
    let want = hex_bytes(
        "789cedcac90140401045c1547e04a291008c9d66685bf484318777aecabba0cdfb6a5419ed5ad4d8adc1e775979d21eaf8792ade47b5b599c86432994c269393ca1f258c27cc",
    );
    let back = decompress_zlib(&z).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "xval-our-dynamic-fox",
        z == want && back.as_deref() == Ok(fox.as_slice()),
        format!("{} bytes dynamic (pinned, CPython-verified)", z.len()),
    ));

    // CPython zlib.compress(fox, 1) -> fixed block, 77 bytes.
    let z1 = hex_bytes(
        "78010bc94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d5228014ae72456552aa4e4a7eb29848c2a1e0d8dd1b4319a53468b82d18271b49a18ad340753ab0000258c27cc",
    );
    let back = decompress_zlib(&z1).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "xval-cpython-fixed-fox-1",
        back.as_deref() == Ok(fox.as_slice()),
        format!("zlib(level 1, fixed) {} bytes -> {} bytes", z1.len(), fox.len()),
    ));

    // CPython zlib.compress(fox, 6) -> fixed block, 72 bytes.
    let z6 = hex_bytes(
        "789c0bc94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d5228014ae72456552aa4e4a7eb29848c2a1e553caa7854f1a8e251c5a38a47150f26c500258c27cc",
    );
    let back = decompress_zlib(&z6).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "xval-cpython-fixed-fox-6",
        back.as_deref() == Ok(fox.as_slice()),
        format!("zlib(level 6, fixed) {} bytes -> {} bytes", z6.len(), fox.len()),
    ));

    // CPython zlib.compress(data, 1) over the incompressible message.
    let zd = hex_bytes(
        "780163d898247ca454ed7a87e5abd93e7fd7c50b1c2c52bed26af67c86e7afd531bcfbf2152e36193f99eaf67d4524d7ee1cd973f5060f27397f591ac6be2353ea748deebd7e878f8b8259b6a6899fa8d4badd63fb6e7e00e3a66491a365ea373aad5ecff1fdb73e41f050b1cad536f31733bd7eaf89e5db5fa078a9d9e4e934f71f2ba3b8f7e4ca9d6f307c34d9e5ebb2708e9d59d2676af5ee4f70fcb43884755bbac4c92aed3bbd76ef1704326d4e113d56ae71b3cbfacd5cbfff1b12850e97a85e6bb77839cbfbcfda38fe03854a975b4c9f4df7f8b92a9a676f9efc8546a3c7535cbf2d8fe0dc952d73b64effc144a7cf4b42d9b667489eaad6b9db67ff616110f39654b1e3159ab7ba6ddecef36718f5ff68fc8fa6ffd1fc3f5afe8d96ffa3f5df68fd3fdafe196dff0d81f62f00a42ed650",
    );
    let back = decompress_zlib(&zd).map_err(|e| format!("{e:?}"));
    v.push(chk(
        "xval-cpython-fixed-data",
        back.as_deref() == Ok(data.as_slice()),
        format!("zlib(level 1) {} bytes -> {} bytes incompressible", zd.len(), data.len()),
    ));

    v
}
