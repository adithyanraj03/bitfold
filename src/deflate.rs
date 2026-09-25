//! The DEFLATE encoder (RFC 1951) and the zlib container (RFC 1950 §2.2).
//!
//! Pipeline: `lzw77` phrases -> 64 KiB input blocks -> per-block choice of
//! BTYPE 00 (stored) / 01 (fixed) / 10 (dynamic) by measured bit cost ->
//! LSB-first bit stream -> zlib wrapper (`0x78 0x9c`, DEFLATE, big-endian
//! Adler-32 trailer).
//!
//! Huffman lengths are built by the plain two-smallest heap algorithm
//! (deterministic tie-break: insertion order). If the resulting maximum
//! length exceeds the format's cap (15 for data trees, 7 for the 19-symbol
//! code-length tree), that option is unavailable for the block and the
//! cheaper of the remaining legal options is used.

use crate::bits::BitWriter;
use crate::huff::{
    canonical_codes, CodeTable, CODE_LENGTH_ORDER, DIST_EXTRA, FIXED_DIST_LENGTHS,
    FIXED_LIT_LENGTHS, LEN_EXTRA, length_code, distance_code, encode_code_lengths,
};
use crate::lzw77::Phrase;

/// Maximum input bytes per block (the LEN field of a stored block is
/// 16-bit; 64 KiB is also the conventional dynamic-block budget).
pub const MAX_BLOCK: usize = 65_535;

/// Per-block encoding statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockInfo {
    /// 0 = stored, 1 = fixed, 2 = dynamic.
    pub btype: u32,
    /// Input bytes covered by the block.
    pub n_bytes: usize,
    /// Bits written for this block (header + payload, no padding).
    pub bits: u32,
    /// Literal phrases in the block.
    pub n_lit: u32,
    /// Match phrases in the block.
    pub n_match: u32,
    /// Bytes reproduced by backward copies.
    pub match_bytes: u32,
}

/// Whole-stream encoder statistics.
#[derive(Debug, Clone, Default)]
pub struct CompressStats {
    pub n_blocks: u32,
    pub n_stored: u32,
    pub n_fixed: u32,
    pub n_dynamic: u32,
    pub in_bytes: usize,
    /// Logical bits of the DEFLATE stream (headers + payloads, no final
    /// byte padding, no container).
    pub deflate_bits: u64,
    pub n_lit: u64,
    pub n_match: u64,
    pub match_bytes: u64,
    pub blocks: Vec<BlockInfo>,
}

/// One symbol to encode: a literal, a (length, distance) match, or the
/// end-of-block marker (literal/length value 256).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sym {
    Lit(u8),
    Match { len: u32, dist: u32 },
    Eob,
}

/// A min-heap of Huffman tree nodes.
struct HuffNode {
    freq: u64,
    seq: u64,
    left: i32,
    right: i32,
}

/// Build Huffman code lengths from symbol frequencies (two-smallest heap
/// algorithm). `None` if the maximum length exceeds `max_len`. Symbols
/// with frequency 0 get length 0; a single-symbol alphabet gets length 1.
///
/// Ties are broken by insertion order, so the result is a pure function
/// of the frequencies.
pub fn huffman_lengths(freq: &[u64], max_len: u32) -> Option<Vec<u8>> {
    let mut nodes: Vec<HuffNode> = Vec::new();
    let mut heap: Vec<i32> = Vec::new();
    // Symbol -> compacted leaf-node index (symbols with freq 0: -1).
    let mut leaf_of: Vec<i32> = vec![-1; freq.len()];
    for (i, &f) in freq.iter().enumerate() {
        if f > 0 {
            leaf_of[i] = nodes.len() as i32;
            nodes.push(HuffNode {
                freq: f,
                seq: i as u64,
                left: -1,
                right: -1,
            });
            heap.push(nodes.len() as i32 - 1);
        }
    }
    if heap.is_empty() {
        return Some(vec![0u8; freq.len()]);
    }
    if heap.len() == 1 {
        // A single symbol: the format wants a length of 1 (one-bit codes
        // are legal in DEFLATE; see the one-distance-code note in §3.2.7).
        let mut out = vec![0u8; freq.len()];
        out[freq.iter().position(|&f| f > 0).unwrap()] = 1;
        return Some(out);
    }
    // Heapify.
    let n0 = heap.len();
    let mut i = n0 / 2 - 1;
    loop {
        if i == 0 {
            break;
        }
        i -= 1;
        sift_down(&mut heap, i, &nodes);
    }
    let mut next_seq = n0 as u64;
    while heap.len() > 1 {
        let a = heap[0];
        pop_root(&mut heap, &nodes);
        let b = heap[0];
        pop_root(&mut heap, &nodes);
        nodes.push(HuffNode {
            freq: nodes[a as usize].freq + nodes[b as usize].freq,
            seq: next_seq,
            left: a,
            right: b,
        });
        next_seq += 1;
        heap.push((nodes.len() - 1) as i32);
        sift_down(&mut heap, 0, &nodes);
    }
    let root = (nodes.len() - 1) as i32;
    // Depth pass (iterative) over all nodes; leaf depths are then read
    // back through `leaf_of`.
    let mut depth = vec![0u32; nodes.len()];
    let mut stack: Vec<(i32, u32)> = vec![(root, 0)];
    while let Some((id, d)) = stack.pop() {
        let node = &nodes[id as usize];
        if node.left >= 0 {
            depth[node.left as usize] = d + 1;
            depth[node.right as usize] = d + 1;
            stack.push((node.left, d + 1));
            stack.push((node.right, d + 1));
        } else {
            depth[id as usize] = d;
        }
    }
    let mut out = vec![0u8; freq.len()];
    for (i, &leaf) in leaf_of.iter().enumerate() {
        if leaf >= 0 {
            out[i] = depth[leaf as usize] as u8;
        }
    }
    if out.iter().copied().max().map(|m| m as u32) > Some(max_len) {
        return None;
    }
    Some(out)
}

fn less_node(a: i32, b: i32, nodes: &[HuffNode]) -> bool {
    (nodes[a as usize].freq, nodes[a as usize].seq) < (nodes[b as usize].freq, nodes[b as usize].seq)
}

fn sift_down(heap: &mut Vec<i32>, mut c: usize, nodes: &[HuffNode]) {
    loop {
        let r = 2 * c + 1;
        if r >= heap.len() {
            break;
        }
        let mut m = r;
        let rr = r + 1;
        if rr < heap.len() && less_node(heap[rr], heap[r], nodes) {
            m = rr;
        }
        if less_node(heap[m], heap[c], nodes) {
            heap.swap(c, m);
            c = m;
        } else {
            break;
        }
    }
}

fn pop_root(heap: &mut Vec<i32>, nodes: &[HuffNode]) {
    let last = heap.pop().unwrap();
    if !heap.is_empty() {
        heap[0] = last;
        sift_down(heap, 0, nodes);
    }
}

/// The fixed-code bit cost of one symbol.
fn fixed_bits(s: &Sym) -> u32 {
    match s {
        Sym::Lit(b) => FIXED_LIT_LENGTHS[*b as usize] as u32,
        Sym::Eob => FIXED_LIT_LENGTHS[256] as u32,
        Sym::Match { len, dist } => {
            let (lc, _extra_l) = length_code(*len);
            let (dc, _extra_d) = distance_code(*dist);
            fixed_lit_len(lc) + LEN_EXTRA[lc as usize] + 5 + DIST_EXTRA[dc as usize]
        }
    }
}

/// Fixed literal/length code length for length code `lc` (257..285):
/// codes 257..279 (lc 0..22) are 7 bits, 280..285 (lc 23..28) are 8 bits.
#[inline]
fn fixed_lit_len(lc: u32) -> u32 {
    if lc < 23 { 7 } else { 8 }
}

/// Trim a length vector at its last non-zero entry (at least `min_len`).
fn trim_lengths(lengths: &[u8], min_len: usize) -> Vec<u8> {
    let mut last = 0usize;
    for (i, &l) in lengths.iter().enumerate() {
        if l != 0 {
            last = i + 1;
        }
    }
    let n = last.max(min_len).min(lengths.len());
    lengths[..n].to_vec()
}

/// Everything needed to emit (or cost) one dynamic block, computed once.
struct DynPlan {
    lit_len: Vec<u8>,
    dist_len: Vec<u8>,
    lit_trim: Vec<u8>,
    dist_trim: Vec<u8>,
    code_syms: Vec<u32>,
    code_reps: Vec<u32>,
    cl_len: Vec<u8>,
    /// How many code-length codes have nonzero length; HCLEN is this
    /// count minus 4 (RFC §3.2.7).
    cl_nonzero: usize,
    header_bits: u32,
    sym_bits: u32,
}

/// Compute the dynamic-block plan (tables + costs). `None` if no legal
/// 15/7-bit tables exist for this symbol mix.
fn dynamic_plan(syms: &[Sym]) -> Option<DynPlan> {
    let mut lit_freq = [0u64; 286];
    let mut dist_freq = [0u64; 30];
    for s in syms {
        match s {
            Sym::Lit(b) => lit_freq[*b as usize] += 1,
            Sym::Eob => lit_freq[256] += 1,
            Sym::Match { len, dist } => {
                let (lc, _) = length_code(*len);
                let (dc, _) = distance_code(*dist);
                lit_freq[257 + lc as usize] += 1;
                dist_freq[dc as usize] += 1;
            }
        }
    }
    let lit_len = huffman_lengths(&lit_freq, 15)?;
    let dist_len = huffman_lengths(&dist_freq, 15)?;
    let lit_trim = trim_lengths(&lit_len, 257);
    let dist_trim = trim_lengths(&dist_len, 1);
    let mut seq = lit_trim.clone();
    seq.extend_from_slice(&dist_trim);
    let (code_syms, code_reps) = encode_code_lengths(&seq);
    let mut cl_freq = [0u64; 19];
    for &s in &code_syms {
        cl_freq[s as usize] += 1;
    }
    let cl_len = huffman_lengths(&cl_freq, 7)?;
    let cl_nonzero = cl_len.iter().filter(|&&l| l != 0).count();
    let header_bits: u32 = 3 + 5 + 5 + 4 + 3 * 19
        + code_syms
            .iter()
            .map(|&s| {
                cl_len[s as usize] as u32
                    + match s {
                        16 => 2,
                        17 => 3,
                        18 => 7,
                        _ => 0,
                    }
            })
            .sum::<u32>();
    let sym_bits: u32 = syms.iter().map(|s| match s {
        Sym::Lit(b) => lit_len[*b as usize] as u32,
        Sym::Eob => lit_len[256] as u32,
        Sym::Match { len, dist } => {
            let (lc, _extra_l) = length_code(*len);
            let (dc, _extra_d) = distance_code(*dist);
            lit_len[257 + lc as usize] as u32
                + LEN_EXTRA[lc as usize]
                + dist_len[dc as usize] as u32
                + DIST_EXTRA[dc as usize]
        }
    }).sum();
    Some(DynPlan {
        lit_len,
        dist_len,
        lit_trim,
        dist_trim,
        code_syms,
        code_reps,
        cl_len,
        cl_nonzero,
        header_bits,
        sym_bits,
    })
}

/// Write a stored block (BTYPE 00). `bytes` must be the exact input bytes
/// of the block (matches are expanded by the caller).
fn write_stored(w: &mut BitWriter, bytes: &[u8], bfinal: u64) {
    w.push_lsb(bfinal, 1);
    w.push_lsb(0b00, 2);
    w.align_to_byte();
    debug_assert!(bytes.len() <= MAX_BLOCK);
    w.push_lsb(bytes.len() as u64, 16);
    w.push_lsb((bytes.len() as u64) ^ 0xFFFF, 16);
    for &b in bytes {
        w.push_lsb(b as u64, 8);
    }
}

/// Write a fixed-code block (BTYPE 01).
fn write_fixed(w: &mut BitWriter, syms: &[Sym], bfinal: u64) {
    w.push_lsb(bfinal, 1);
    w.push_lsb(0b01, 2);
    let lit_codes = canonical_codes(&FIXED_LIT_LENGTHS, 9);
    let dist_codes = canonical_codes(&FIXED_DIST_LENGTHS, 5);
    for s in syms {
        match s {
            Sym::Lit(b) => {
                w.push_huff(lit_codes[*b as usize], FIXED_LIT_LENGTHS[*b as usize] as u32);
            }
            Sym::Eob => {
                w.push_huff(lit_codes[256], FIXED_LIT_LENGTHS[256] as u32);
            }
            Sym::Match { len, dist } => {
                let (lc, extra_l) = length_code(*len);
                let (dc, extra_d) = distance_code(*dist);
                w.push_huff(lit_codes[257 + lc as usize], fixed_lit_len(lc));
                // Extra bits per the §3.2.5 prose: MSB-first.
                w.push_huff(extra_l, LEN_EXTRA[lc as usize]);
                w.push_huff(dist_codes[dc as usize], 5);
                w.push_huff(extra_d, DIST_EXTRA[dc as usize]);
            }
        }
    }
}

/// Write a dynamic-code block (BTYPE 10) from a prepared plan.
fn write_dynamic(w: &mut BitWriter, plan: &DynPlan, syms: &[Sym], bfinal: u64) {
    w.push_lsb(bfinal, 1);
    w.push_lsb(0b10, 2);
    w.push_lsb((plan.lit_trim.len() - 257) as u64, 5);
    w.push_lsb((plan.dist_trim.len() - 1) as u64, 5);
    // HCLEN = number of nonzero code length codes - 4 (RFC §3.2.7).
    w.push_lsb((plan.cl_nonzero - 4) as u64, 4);
    for &sym_idx in CODE_LENGTH_ORDER.iter() {
        w.push_lsb(plan.cl_len[sym_idx as usize] as u64, 3);
    }
    let cl_codes = canonical_codes(&plan.cl_len, 7);
    for (i, &s) in plan.code_syms.iter().enumerate() {
        w.push_huff(cl_codes[s as usize], plan.cl_len[s as usize] as u32);
        match s {
            16 => w.push_lsb(plan.code_reps[i] as u64, 2),
            17 => w.push_lsb(plan.code_reps[i] as u64, 3),
            18 => w.push_lsb(plan.code_reps[i] as u64, 7),
            _ => {}
        }
    }
    let lit_codes = canonical_codes(&plan.lit_trim, 15);
    let dist_codes = canonical_codes(&plan.dist_trim, 15);
    for s in syms {
        match s {
            Sym::Lit(b) => {
                w.push_huff(lit_codes[*b as usize], plan.lit_len[*b as usize] as u32);
            }
            Sym::Eob => {
                w.push_huff(lit_codes[256], plan.lit_len[256] as u32);
            }
            Sym::Match { len, dist } => {
                let (lc, extra_l) = length_code(*len);
                let (dc, extra_d) = distance_code(*dist);
                w.push_huff(
                    lit_codes[257 + lc as usize],
                    plan.lit_len[257 + lc as usize] as u32,
                );
                // Extra bits per the §3.2.5 prose: MSB-first.
                w.push_huff(extra_l, LEN_EXTRA[lc as usize]);
                w.push_huff(dist_codes[dc as usize], plan.dist_len[dc as usize] as u32);
                w.push_huff(extra_d, DIST_EXTRA[dc as usize]);
            }
        }
    }
}

/// Expand phrases to the byte sequence they produce (one-byte-at-a-time
/// copy, overlaps legal — identical to what the decoder does).
#[must_use]
pub fn phrases_to_bytes(phrases: &[Phrase]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for p in phrases {
        match p {
            Phrase::Literal(b) => out.push(*b),
            Phrase::Match { len, dist } => {
                debug_assert!(*dist >= 1 && *dist <= 32_768);
                for _ in 0..*len {
                    let b = out[out.len() - *dist as usize];
                    out.push(b);
                }
            }
        }
    }
    out
}

/// Compress `data` to a raw DEFLATE stream (no zlib container) and record
/// per-block statistics.
pub fn compress_raw(data: &[u8]) -> (Vec<u8>, CompressStats) {
    let phrases = crate::lzw77::compress(data);
    let mut stats = CompressStats {
        in_bytes: data.len(),
        ..Default::default()
    };
    let mut w = BitWriter::new();

    // Group phrases into blocks by input bytes covered. Each block
    // records its input offset: a stored block must cover the *raw input
    // bytes* of its span (a match may reference bytes before the block
    // start, which a phrase-only expansion could not reproduce).
    let mut blocks: Vec<(Vec<Phrase>, usize, usize)> = Vec::new();
    let mut cur: Vec<Phrase> = Vec::new();
    let mut cur_bytes = 0usize;
    let mut cur_off = 0usize;
    for p in &phrases {
        let pb = match p {
            Phrase::Literal(_) => 1usize,
            Phrase::Match { len, .. } => *len as usize,
        };
        if cur_bytes + pb > MAX_BLOCK && !cur.is_empty() {
            blocks.push((std::mem::take(&mut cur), cur_bytes, cur_off));
            cur_off += cur_bytes;
            cur_bytes = 0;
        }
        cur.push(*p);
        cur_bytes += pb;
    }
    if !cur.is_empty() || blocks.is_empty() {
        blocks.push((cur, cur_bytes, cur_off));
    }

    for (bi, block) in blocks.iter().enumerate() {
        let bp = &block.0;
        let nbytes = block.1;
        let off = block.2;
        let syms: Vec<Sym> = bp
            .iter()
            .map(|p| match p {
                Phrase::Literal(b) => Sym::Lit(*b),
                Phrase::Match { len, dist } => Sym::Match { len: *len, dist: *dist },
            })
            .chain(std::iter::once(Sym::Eob))
            .collect();
        let bfinal = if bi == blocks.len() - 1 { 1 } else { 0 };
        let before = w.logical_bits();

        // Costs. A block header is exactly 3 bits, so a stored block
        // always pays 5 padding bits before its LEN/NLEN pair:
        // 3 + 5 + 16 + 8*n_bytes.
        let stored_bits = 3u32 + 5 + 16 + (nbytes as u32) * 8;
        let fixed_cost = 3 + syms.iter().map(fixed_bits).sum::<u32>();
        let dyn_plan = dynamic_plan(&syms);
        let dyn_cost = dyn_plan.as_ref().map(|p| p.header_bits + p.sym_bits);

        let btype = if let Some(dc) = dyn_cost {
            if dc <= fixed_cost && dc <= stored_bits {
                2
            } else if fixed_cost <= stored_bits {
                1
            } else {
                0
            }
        } else if fixed_cost <= stored_bits {
            1
        } else {
            0
        };

        match btype {
            0 => {
                let bytes = &data[off..off + nbytes];
                write_stored(&mut w, bytes, bfinal);
            }
            1 => write_fixed(&mut w, &syms, bfinal),
            _ => {
                if let Some(plan) = dyn_plan {
                    write_dynamic(&mut w, &plan, &syms, bfinal);
                }
            }
        }

        let (nl, nm) = (
            syms.iter().filter(|s| matches!(s, Sym::Lit(_))).count() as u32,
            syms.iter().filter(|s| matches!(s, Sym::Match { .. })).count() as u32,
        );
        let mb: u32 = syms
            .iter()
            .filter_map(|s| if let Sym::Match { len, .. } = s { Some(*len) } else { None })
            .sum();
        let bits = w.logical_bits() - before;
        let info = BlockInfo {
            btype,
            n_bytes: nbytes,
            bits: bits as u32,
            n_lit: nl,
            n_match: nm,
            match_bytes: mb,
        };
        stats.n_blocks += 1;
        match btype {
            0 => stats.n_stored += 1,
            1 => stats.n_fixed += 1,
            _ => stats.n_dynamic += 1,
        }
        stats.n_lit += nl as u64;
        stats.n_match += nm as u64;
        stats.match_bytes += mb as u64;
        stats.deflate_bits += bits as u64;
        stats.blocks.push(info);
    }
    (w.finish(), stats)
}

/// Compress `data` into the zlib container: `0x78 0x9c` + DEFLATE +
/// big-endian Adler-32 (RFC 1950 §2.2).
#[must_use]
pub fn compress_zlib(data: &[u8]) -> Vec<u8> {
    let (deflate, _stats) = compress_raw(data);
    let mut out: Vec<u8> = Vec::with_capacity(deflate.len() + 6);
    out.push(0x78);
    out.push(0x9c);
    out.extend_from_slice(&deflate);
    out.extend_from_slice(&crate::adler::adler32_fresh(data).to_be_bytes());
    out
}

/// The container hashes (CRC-32 + Adler-32) of `data` — pinned in KATs.
#[must_use]
pub fn container_hashes(data: &[u8]) -> (u32, u32) {
    (crate::crc32::crc32_fresh(data), crate::adler::adler32_fresh(data))
}

/// The pre-built fixed-code tables (exposed for KATs and the decoder).
#[must_use]
pub fn fixed_lit_table() -> CodeTable {
    CodeTable::build(&FIXED_LIT_LENGTHS)
}

#[must_use]
pub fn fixed_dist_table() -> CodeTable {
    CodeTable::build(&FIXED_DIST_LENGTHS)
}

