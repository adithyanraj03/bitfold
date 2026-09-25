//! Bitfold — RFC 1951 DEFLATE from scratch: LZ77 matching, canonical
//! Huffman coding, and the zlib container hashes, in std-only Rust.

pub mod adler;
pub mod bits;
pub mod corpus;
pub mod crc32;
pub mod crypto;
pub mod huff;
/// Embedded KAT data, auto-generated from `Deleted_files/refs/
/// zlib_hash_tests.h` by `tools/gen_kat_data.py` (see that file for the
/// extraction and CPython verification provenance).
pub mod kat_data;

/// Crate version (pinned).
pub const VERSION: &str = "1.0.0";
