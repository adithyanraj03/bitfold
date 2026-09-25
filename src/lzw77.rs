//! LZ77 phrase search: chained hash table on 3-byte sequences with lazy
//! matching — the algorithm RFC 1951 §4 recommends (and notes is known
//! not to be patented per se).
//!
//! * Hash table: 2^16 chains, `head[hash]` = most recent position,
//!   `prev[pos]` = previous position with the same hash. Chains are
//!   singly linked, never deleted; matches older than the 32 KiB window
//!   are discarded, and chains are arbitrarily truncated at 128 entries.
//! * Search order: most recent first (favors small distances, which the
//!   Huffman code then exploits).
//! * Lazy matching: a match found at position `p` is held while the next
//!   position is searched; if the next match is longer, `p` is emitted as
//!   a literal and the longer match takes over, otherwise the held match
//!   is emitted (advancing by its full length).
//!
//! Match validity mirrors the decoder exactly: distance 1..=32768, length
//! 3..=258, and overlaps (distance < length) are legal — the encoder's
//! extension loop compares `input[candidate + i]` against
//! `input[pos + i]` byte by byte, which is precisely what a one-byte-at-a
//! time decoder copy produces.

/// The backward-match window (a compliant encoder may limit this; we use
/// the full range a compliant decoder must accept).
pub const WINDOW: usize = 32_768;
/// Longest encodable match.
pub const MAX_MATCH: u32 = 258;
/// Shortest match worth encoding.
pub const MIN_MATCH: u32 = 3;
/// Arbitrary chain truncation (RFC §4: "very long hash chains are
/// arbitrarily truncated at a certain length").
pub const MAX_CHAIN: usize = 128;
/// Hash table size (2^16 chains for 3-byte sequences).
pub const HASH_SIZE: usize = 1 << 16;

/// One output phrase: a literal byte or a backward match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phrase {
    /// A literal byte (no compression for this byte).
    Literal(u8),
    /// Copy `len` (3..=258) bytes from `dist` (1..=32768) bytes back;
    /// the copy may overlap (dist < len).
    Match { len: u32, dist: u32 },
}

/// The LZ77 search state over one input.
pub struct Lz77 {
    /// Hash chains (heap-allocated: 512 KiB does not belong on a stack).
    head: Vec<i64>,
    prev: Vec<i64>,
    input: Vec<u8>,
}

impl Lz77 {
    /// A search state over `input` (the input is copied; the window is
    /// bounded by `WINDOW` backward references).
    pub fn new(input: &[u8]) -> Self {
        Self {
            head: vec![-1; HASH_SIZE],
            prev: vec![-1; input.len()],
            input: input.to_vec(),
        }
    }

    /// 16-bit hash of the 3 bytes at `i` (i + 3 <= input.len()).
    ///
    /// `((b0 << 6) + b1 + (b2 << 6)) mod 2^16` — zlib's HASH_HEAD shape
    /// with HASH_BITS = 16.
    fn hash_at(&self, i: usize) -> usize {
        let b0 = self.input[i] as u32;
        let b1 = self.input[i + 1] as u32;
        let b2 = self.input[i + 2] as u32;
        (((b0 << 6) + b1 + (b2 << 6)) & 0xFFFF) as usize
    }

    /// Longest match at `p` over the current chains (candidates strictly
    /// before `p`); `(0, 0)` if none of length >= 3.
    fn search(&self, p: usize) -> (u32, u32) {
        let n = self.input.len();
        if p + 3 > n {
            return (0, 0);
        }
        let h = self.hash_at(p);
        let max_len = ((n - p) as u32).min(MAX_MATCH);
        let mut best_len = 0u32;
        let mut best_dist = 0u32;
        let mut cand = self.head[h];
        let mut chain = 0usize;
        while cand >= 0 && chain < MAX_CHAIN {
            let cp = cand as usize;
            if p - cp > WINDOW {
                break; // older matches than the window are unusable
            }
            let mut l = 0u32;
            while l < max_len && self.input[cp + l as usize] == self.input[p + l as usize] {
                l += 1;
            }
            if l > best_len {
                best_len = l;
                best_dist = (p - cp) as u32;
                if l == max_len {
                    break;
                }
            }
            cand = self.prev[cp];
            chain += 1;
        }
        if best_len >= MIN_MATCH {
            (best_len, best_dist)
        } else {
            (0, 0)
        }
    }

    /// Run the full compression pass: returns the phrase list.
    pub fn phrases(&mut self) -> Vec<Phrase> {
        let n = self.input.len();
        let mut out: Vec<Phrase> = Vec::new();
        let mut pos = 0usize;
        // Held lazy match: (start position, len, dist).
        let mut pending: Option<(usize, u32, u32)> = None;

        while pos < n {
            // Insert the hash for the current position (only positions
            // strictly before `pos` are search candidates, so insert
            // after searching).
            if pos + 3 <= n {
                let (len, dist) = self.search(pos);
                let h = self.hash_at(pos);
                self.prev[pos] = self.head[h];
                self.head[h] = pos as i64;
                match pending {
                    Some((pp, plen, pdist)) => {
                        if len > plen {
                            // Longer match at the current position: the
                            // held match's first byte becomes a literal.
                            out.push(Phrase::Literal(self.input[pp]));
                            pending = if len >= MIN_MATCH {
                                Some((pos, len, dist))
                            } else {
                                None
                            };
                            pos += 1;
                        } else {
                            // Keep the held match (equal or longer is not
                            // available): emit it, skip its span, and
                            // backfill the hash entries of the skipped
                            // positions (they are valid match sources for
                            // future phrases, most recent first).
                            out.push(Phrase::Match { len: plen, dist: pdist });
                            let end = pp + plen as usize;
                            for p in (pp + 1)..end {
                                if p + 3 <= n {
                                    let h = self.hash_at(p);
                                    self.prev[p] = self.head[h];
                                    self.head[h] = p as i64;
                                }
                            }
                            pos = end;
                            pending = None;
                        }
                    }
                    None => {
                        if len >= MIN_MATCH {
                            // Hold it; the next position decides.
                            pending = Some((pos, len, dist));
                            pos += 1;
                        } else {
                            out.push(Phrase::Literal(self.input[pos]));
                            pos += 1;
                        }
                    }
                }
            } else {
                // Fewer than 3 bytes remain: no new search possible.
                match pending.take() {
                    Some((pp, plen, pdist)) => {
                        out.push(Phrase::Match { len: plen, dist: pdist });
                        pos = pp + plen as usize;
                    }
                    None => {
                        out.push(Phrase::Literal(self.input[pos]));
                        pos += 1;
                    }
                }
            }
        }

        // Drain a pending match at the very end (it started earlier and
        // its span fits, since search() only returns lengths that fit).
        if let Some((pp, plen, pdist)) = pending {
            debug_assert!(pp + plen as usize <= n);
            out.push(Phrase::Match { len: plen, dist: pdist });
        }
        out
    }
}

/// Convenience: phrase-search `input` in one call.
#[must_use]
pub fn compress(input: &[u8]) -> Vec<Phrase> {
    let mut lz = Lz77::new(input);
    lz.phrases()
}

/// The number of literal / match phrases in a phrase list.
#[must_use]
pub fn phrase_stats(phrases: &[Phrase]) -> (usize, usize) {
    let (mut lit, mut mat) = (0usize, 0usize);
    for p in phrases {
        match p {
            Phrase::Literal(_) => lit += 1,
            Phrase::Match { .. } => mat += 1,
        }
    }
    (lit, mat)
}
