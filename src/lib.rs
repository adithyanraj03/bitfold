//! Bitfold — RFC 1951 DEFLATE from scratch: LZ77 matching, canonical
//! Huffman coding, and the zlib container hashes, in std-only Rust.

pub mod corpus;

/// Crate version (pinned).
pub const VERSION: &str = "1.0.0";
