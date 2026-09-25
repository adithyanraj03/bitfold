//! # Bitfold
//!
//! RFC 1951 DEFLATE — LZ77 (chained hash + lazy matching) and canonical
//! Huffman coding (stored / fixed / dynamic blocks, LSB-first bit packing)
//! — plus the two zlib container hashes (CRC-32 and Adler-32), rebuilt
//! from scratch in `std`-only Rust and verified against the full zlib
//! test battery and independent round-trips.
//!
//! The name is the story: entropy *folds* the data down. The crate
//! measures exactly how far — a u128 fixed-point Shannon entropy ledger
//! per corpus part, contrasted against the bits the encoder actually
//! emits.

pub mod adler;
pub mod battery;
pub mod bits;
pub mod corpus;
pub mod crc32;
pub mod crypto;
pub mod deflate;
pub mod entropy;
pub mod huff;
pub mod inflate;
pub mod kats;
/// Embedded KAT data, auto-generated from `Deleted_files/refs/
/// zlib_hash_tests.h` by `tools/gen_kat_data.py` (see that file for the
/// extraction and CPython verification provenance).
pub mod kat_data;
pub mod lzw77;
pub mod svg;

/// Crate version (pinned; also printed by `bitfold version`).
pub const VERSION: &str = "1.0.0";
