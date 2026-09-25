//! Smoke test: the encoder/decoder round-trip on a handful of shapes.
#![allow(unused)]

use bitfold::corpus;
use bitfold::deflate::{compress_raw, compress_zlib, container_hashes, phrases_to_bytes};
use bitfold::inflate::{decompress_raw, decompress_zlib};

#[test]
fn small_roundtrip() {
    let msgs: [&[u8]; 6] = [
        b"",
        b"A",
        b"hello world",
        b"The quick brown fox jumps over the lazy dog.",
        &[0u8; 100],
        &vec![0x42; 1000],
    ];
    for msg in msgs {
        let z = compress_zlib(msg);
        let back = decompress_zlib(&z).expect("zlib round-trip");
        assert_eq!(back, msg, "round-trip failed for len {}", msg.len());
    }
}

#[test]
fn raw_roundtrip_corpus() {
    let data = corpus::assemble();
    let (raw, stats) = compress_raw(&data);
    let back = decompress_raw(&raw).expect("raw round-trip");
    assert_eq!(back.len(), data.len());
    assert_eq!(back, data, "corpus round-trip mismatch");
    // Sanity on the stats.
    assert!(stats.n_blocks >= 3, "expected multiple 64KiB blocks, got {}", stats.n_blocks);
    assert!(stats.n_dynamic > 0 || stats.n_fixed > 0);
    let (crc, adler) = container_hashes(&data);
    assert_eq!(adler, bitfold::adler::adler32_fresh(&data));
    assert_eq!(crc, bitfold::crc32::crc32_fresh(&data));
}

#[test]
fn phrases_reconstruct() {
    // A repetitive message must produce matches that reconstruct it.
    let msg: Vec<u8> = "abcdefabcdefabcdefabcdef".repeat(40).into();
    let phrases = bitfold::lzw77::compress(&msg);
    let (nl, nm) = bitfold::lzw77::phrase_stats(&phrases);
    assert!(nm > 0, "expected matches for repetitive input");
    assert!(nl < msg.len(), "expected compression");
    let back = phrases_to_bytes(&phrases);
    assert_eq!(back, msg);
}

#[test]
fn stored_block_roundtrip() {
    // Incompressible-ish data may be stored; ensure stored blocks decode.
    let msg: Vec<u8> = (0..200u32).map(|i| ((i * 131 + 7) & 0xFF) as u8).collect();
    let z = compress_zlib(&msg);
    let back = decompress_zlib(&z).expect("stored round-trip");
    assert_eq!(back, msg);
}
