//! Bit-packer tests: the two element orders, boundaries, exhaustion.

use bitfold::bits::{BitError, BitReader, BitWriter};

#[test]
fn lsb_roundtrip_many() {
    let cases: &[(u64, u32)] = &[
        (1, 1),
        (0, 3),
        (5, 3),
        (10, 4),
        (255, 8),
        (0xABCD, 16),
        (u32::MAX as u64, 32),
        (0xDEAD_BEEF_CAFE_1234, 64),
        (1, 64),
        (0, 64),
    ];
    for &(v, n) in cases {
        let mut w = BitWriter::new();
        w.push_lsb(v, n);
        let out = w.finish();
        let mut r = BitReader::new(&out);
        let back = r.next_lsb(n).unwrap();
        assert_eq!(back, v, "lsb {n}-bit roundtrip of {v:#x}");
    }
}

#[test]
fn huff_roundtrip_many() {
    let cases: &[(u32, u32)] = &[(1, 1), (0, 3), (5, 3), (10, 4), (255, 8), (0xABCD, 16), (u32::MAX, 32)];
    for &(v, n) in cases {
        let mut w = BitWriter::new();
        w.push_huff(v, n);
        let out = w.finish();
        let mut r = BitReader::new(&out);
        let back = r.next_huff(n).unwrap();
        assert_eq!(back, v, "huff {n}-bit roundtrip of {v:#x}");
    }
}

#[test]
fn lsb_and_huff_diverge_on_asymmetric_values() {
    // 0b110 in 3 bits: lsb order 0,1,1 vs huff order 1,1,0 — different bytes.
    let mut a = BitWriter::new();
    a.push_lsb(0b110, 3);
    a.push_lsb(0b1010, 4);
    a.push_lsb(1, 1);
    let mut b = BitWriter::new();
    b.push_huff(0b110, 3);
    b.push_huff(0b1010, 4);
    b.push_huff(1, 1);
    let la = a.finish();
    let lb = b.finish();
    assert_eq!(la, [0xD6]);
    assert_eq!(lb, [0xAB]);
    assert_ne!(la, lb);
}

#[test]
fn mixed_sequence_roundtrip() {
    let mut w = BitWriter::new();
    w.push_lsb(0b101, 3);
    w.push_huff(0b1011, 4);
    w.push_lsb(0x1234, 16);
    w.push_huff(0xAB, 8);
    w.push_lsb(7, 3);
    assert_eq!(w.logical_bits(), 34);
    let out = w.finish();
    assert_eq!(out.len(), 5); // 34 logical bits -> 5 bytes
    let mut r = BitReader::new(&out);
    assert_eq!(r.next_lsb(3).unwrap(), 0b101);
    assert_eq!(r.next_huff(4).unwrap(), 0b1011);
    assert_eq!(r.next_lsb(16).unwrap(), 0x1234);
    assert_eq!(r.next_huff(8).unwrap(), 0xAB);
    assert_eq!(r.next_lsb(3).unwrap(), 7);
    assert!(!r.at_end()); // 2 padding bits remain
}

#[test]
fn reader_exhaustion_is_an_error() {
    let data = [0b1000_0000, 0b01];
    let mut r = BitReader::new(&data);
    assert_eq!(r.next_lsb(8).unwrap(), 0b1000_0000);
    assert_eq!(r.next_lsb(1).unwrap(), 1); // bit0 of 0x01
    assert_eq!(r.next_lsb(7).unwrap(), 0); // bits 1..7 of 0x01
    assert!(matches!(r.next_lsb(1), Err(BitError::Exhausted)));
    let mut r = BitReader::new(&[0u8; 0]);
    assert!(matches!(r.next_lsb(1), Err(BitError::Exhausted)));
    assert!(matches!(r.next_huff(1), Err(BitError::Exhausted)));
}

#[test]
fn writer_finish_pads_partial_byte_with_zeros() {
    let mut w = BitWriter::new();
    w.push_lsb(0b101, 3);
    let out = w.finish();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], 0b101); // bits 0-2 set, 3-7 zero
    let mut w = BitWriter::new();
    w.push_huff(0b1, 1);
    let out = w.finish();
    assert_eq!(out, [0x01]); // one code bit -> stream bit 0 -> byte 0x01
}

#[test]
fn align_to_byte_skips_padding() {
    let mut w = BitWriter::new();
    w.push_lsb(0b101, 3);
    w.align_to_byte();
    w.push_lsb(0xAB, 8);
    let out = w.finish();
    assert_eq!(out, [0b101, 0xAB]);
    let mut r = BitReader::new(&out);
    let _ = r.next_lsb(3).unwrap();
    r.align_to_byte();
    assert_eq!(r.bits_read(), 8);
    assert_eq!(r.next_lsb(8).unwrap(), 0xAB);
}

#[test]
fn byte_offset_and_window_after_align() {
    let data = [0x01, 0x02, 0x03];
    let mut r = BitReader::new(&data);
    let _ = r.next_lsb(3).unwrap();
    r.align_to_byte();
    assert_eq!(r.byte_offset(), 1);
    assert_eq!(r.window(), [0x02, 0x03]);
    r.skip_bytes(1);
    assert_eq!(r.byte_offset(), 2);
    assert_eq!(r.next_lsb(8).unwrap(), 0x03);
}

#[test]
fn huff_32_bit_bits_reversed_within_bytes() {
    // A 32-bit MSB-first code packs LSB-first within each output byte, so
    // each byte is the corresponding 8 code bits, bit-reversed.
    let mut w = BitWriter::new();
    w.push_huff(0x1234_5678, 32);
    let out = w.finish();
    assert_eq!(out.len(), 4);
    // 0x12 = 00010010 -> reversed 01001000 = 0x48, etc.
    fn rev8(b: u8) -> u8 {
        let mut v = 0u8;
        for i in 0..8 {
            v |= ((b >> i) & 1) << (7 - i);
        }
        v
    }
    for (i, b) in [0x12u8, 0x34, 0x56, 0x78].iter().enumerate() {
        assert_eq!(out[i], rev8(*b));
    }
}

#[test]
fn logical_bits_tracks_only_payload() {
    let mut w = BitWriter::new();
    w.push_lsb(1, 3);
    w.push_lsb(1, 3);
    assert_eq!(w.logical_bits(), 6);
    w.align_to_byte();
    // align_to_byte must not inflate the logical count
    assert_eq!(w.logical_bits(), 6);
    assert_eq!(w.len(), 1);
}
