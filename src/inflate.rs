//! The DEFLATE decoder (RFC 1951) and the zlib container (RFC 1950 §2.2).
//!
//! Accepts the full range a compliant decompressor must handle: stored,
//! fixed, and dynamic blocks of arbitrary size; backward distances up to
//! 32768 crossing block boundaries; overlapping copies (distance <
//! length). The zlib wrapper is verified (CMF/FLG check bits) and the
//! big-endian Adler-32 trailer is checked.

use crate::bits::BitReader;
use crate::huff::{CodeTable, CODE_LENGTH_ORDER, DIST_BASE, LEN_BASE};

/// A decode failure (malformed stream or checksum mismatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateError {
    /// The stream ended before the data was complete.
    Truncated,
    /// A field was outside its legal range (LEN/NLEN mismatch, BTYPE 11,
    /// overfull code table, distance past the output start, ...).
    BadFormat,
    /// The Adler-32 trailer (zlib container) did not match.
    ChecksumMismatch,
}

/// The fixed literal/length table (RFC §3.2.6).
fn fixed_lit() -> CodeTable {
    static ONCE: std::sync::OnceLock<CodeTable> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| CodeTable::build(&crate::huff::FIXED_LIT_LENGTHS)).clone()
}

/// The fixed distance table (RFC §3.2.6).
fn fixed_dist() -> CodeTable {
    static ONCE: std::sync::OnceLock<CodeTable> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| CodeTable::build(&crate::huff::FIXED_DIST_LENGTHS)).clone()
}

/// Decode a Huffman symbol: consume bits one at a time, walking the code
/// lengths until a match is found (at most `max_len` bits).
fn decode_symbol(r: &mut BitReader, table: &CodeTable) -> Result<usize, InflateError> {
    let max_len = table.max_length() as usize;
    if max_len == 0 {
        return Err(InflateError::BadFormat);
    }
    let mut code = 0u32;
    for len in 1..=max_len {
        code = (code << 1) | (r.next_huff(1).map_err(|_| InflateError::Truncated)?) as u32;
        if let Some(sym) = table.decode(code, len as u32) {
            return Ok(sym);
        }
    }
    Err(InflateError::BadFormat)
}

/// Decode the 19-symbol code-length table from the 3-bit lengths.
/// Only the first `hclen + 4` entries of `CODE_LENGTH_ORDER` are
/// transmitted (the rest are zero), so `hclen` must be passed in.
fn code_length_table(r: &mut BitReader, hclen: usize) -> Result<CodeTable, InflateError> {
    let count = hclen + 4;
    if count > 19 {
        return Err(InflateError::BadFormat);
    }
    let mut len = [0u8; 19];
    for &sym in CODE_LENGTH_ORDER.iter().take(count) {
        let l = r.next_lsb(3).map_err(|_| InflateError::Truncated)?;
        len[sym as usize] = l as u8;
    }
    let table = CodeTable::build(&len);
    // Validity: the table must not be overfull (a prefix code must be
    // decodable); underfull is legal (unused codes).
    check_prefix(&table.lengths)?;
    Ok(table)
}

/// Kraft check: sum of 2^(max_len - l) over used codes must be <= 2^max_len.
fn check_prefix(lengths: &[u8]) -> Result<(), InflateError> {
    let max_len = lengths.iter().copied().max().unwrap_or(0) as u32;
    let mut sum = 0u64;
    for &l in lengths {
        if l > 0 {
            sum += 1u64 << (max_len - l as u32);
        }
    }
    if sum > 1u64 << max_len {
        return Err(InflateError::BadFormat);
    }
    Ok(())
}

/// Decode one dynamic literal/length + distance pair of code lengths,
/// build the tables, and run the symbol loop until end-of-block,
/// appending to `out`.
fn run_dynamic_block(
    r: &mut BitReader,
    out: &mut Vec<u8>,
) -> Result<(), InflateError> {
    let hlit = r.next_lsb(5).map_err(|_| InflateError::Truncated)? as usize;
    let hdist = r.next_lsb(5).map_err(|_| InflateError::Truncated)? as usize;
    let hclen = r.next_lsb(4).map_err(|_| InflateError::Truncated)? as usize;
    let n_lit = hlit + 257;
    let n_dist = hdist + 1;
    let n_clen = hclen + 4;
    // RFC bounds: HLIT <= 31 (288 lit/len lengths), HDIST <= 30 (31... 30
    // distance lengths is the RFC max, HDIST <= 29), HCLEN <= 15.
    if n_lit > 288 || n_dist > 30 || n_clen > 19 {
        return Err(InflateError::BadFormat);
    }
    let cl_table = code_length_table(r, hclen)?;
    // Decode the code-length sequence (16/17/18 runs) inline.
    let mut lens: Vec<u8> = Vec::with_capacity(n_lit + n_dist);
    while lens.len() < n_lit + n_dist {
        let s = decode_symbol(r, &cl_table)? as usize;
        match s {
            0..=15 => lens.push(s as u8),
            16 => {
                if lens.is_empty() {
                    return Err(InflateError::BadFormat);
                }
                let prev = *lens.last().unwrap();
                let rep = r
                    .next_lsb(2)
                    .map_err(|_| InflateError::Truncated)?
                    as usize
                    + 3;
                for _ in 0..rep {
                    if lens.len() < n_lit + n_dist {
                        lens.push(prev);
                    }
                }
            }
            17 => {
                let rep = r
                    .next_lsb(3)
                    .map_err(|_| InflateError::Truncated)?
                    as usize
                    + 3;
                for _ in 0..rep {
                    if lens.len() < n_lit + n_dist {
                        lens.push(0);
                    }
                }
            }
            18 => {
                let rep = r
                    .next_lsb(7)
                    .map_err(|_| InflateError::Truncated)?
                    as usize
                    + 11;
                for _ in 0..rep {
                    if lens.len() < n_lit + n_dist {
                        lens.push(0);
                    }
                }
            }
            _ => return Err(InflateError::BadFormat),
        }
    }
    let lit_len: Vec<u8> = lens[..n_lit].to_vec();
    let dist_len: Vec<u8> = lens[n_lit..].to_vec();
    let lit_table = CodeTable::build(&lit_len);
    let dist_table = CodeTable::build(&dist_len);
    check_prefix(&lit_len)?;
    check_prefix(&dist_len)?;
    // A distance code must exist iff a length code 257..285 may occur;
    // the format only requires a valid prefix code, which is checked above.
    run_symbol_loop(r, out, &lit_table, &dist_table)?;
    Ok(())
}

/// The shared symbol loop (fixed and dynamic blocks share it).
fn run_symbol_loop(
    r: &mut BitReader,
    out: &mut Vec<u8>,
    lit_table: &CodeTable,
    dist_table: &CodeTable,
) -> Result<bool, InflateError> {
    loop {
        let sym = decode_symbol(r, lit_table)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(true);
        } else {
            let lc = sym as u32 - 257;
            if lc > 28 {
                return Err(InflateError::BadFormat);
            }
            // Extra bits per the §3.2.5 prose: MSB-first.
            let extra = r
                .next_huff(crate::huff::LEN_EXTRA[lc as usize])
                .map_err(|_| InflateError::Truncated)?;
            let len = LEN_BASE[lc as usize] + extra;
            let dsym = decode_symbol(r, dist_table)? as u32;
            if dsym > 29 {
                return Err(InflateError::BadFormat);
            }
            let dextra = r
                .next_huff(crate::huff::DIST_EXTRA[dsym as usize])
                .map_err(|_| InflateError::Truncated)?;
            let dist = (DIST_BASE[dsym as usize] + dextra) as usize;
            if dist == 0 || dist > out.len() {
                return Err(InflateError::BadFormat);
            }
            // One-byte-at-a-time copy: legal across block boundaries and
            // overlaps (distance < length).
            for _ in 0..len {
                let b = out[out.len() - dist];
                out.push(b);
            }
        }
    }
}

/// Decode a raw DEFLATE stream (no container).
pub fn decompress_raw(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    decompress_raw_impl(data, None)
}

/// Decode a raw DEFLATE stream, recording per-block diagnostics in
/// `log` (dev/attestation use; the stream interpretation is identical).
pub fn decompress_raw_debug(data: &[u8], log: &mut Vec<String>) -> Result<Vec<u8>, InflateError> {
    decompress_raw_impl(data, Some(log))
}

fn decompress_raw_impl(
    data: &[u8],
    mut log: Option<&mut Vec<String>>,
) -> Result<Vec<u8>, InflateError> {
    let mut r = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();
    let mut finished = false;
    let mut block_no = 0u32;
    while !finished {
        let bfinal = r.next_lsb(1).map_err(|_| InflateError::Truncated)?;
        let btype = r.next_lsb(2).map_err(|_| InflateError::Truncated)?;
        if let Some(lg) = log.as_mut() {
            lg.push(format!(
                "block {} header: bfinal={} btype={} (bit {})",
                block_no, bfinal, btype, r.bits_read()
            ));
        }
        match btype {
            0b00 => {
                r.align_to_byte();
                let len = r.next_lsb(16).map_err(|_| InflateError::Truncated)? as usize;
                let nlen = r.next_lsb(16).map_err(|_| InflateError::Truncated)? as usize;
                if let Some(lg) = log.as_mut() {
                    lg.push(format!(
                        "  stored: len={} nlen={:#x} (bit {})",
                        len, nlen, r.bits_read()
                    ));
                }
                if len ^ nlen != 0xFFFF {
                    return Err(InflateError::BadFormat);
                }
                if r.byte_offset() + len > r.data_len() {
                    return Err(InflateError::Truncated);
                }
                out.extend_from_slice(&r.window()[..len]);
                r.skip_bytes(len);
            }
            0b01 => {
                run_symbol_loop(&mut r, &mut out, &fixed_lit(), &fixed_dist())?;
            }
            0b10 => {
                run_dynamic_block(&mut r, &mut out)?;
            }
            _ => return Err(InflateError::BadFormat),
        }
        if let Some(lg) = log.as_mut() {
            lg.push(format!(
                "  block {} done: out={} bytes (bit {})",
                block_no,
                out.len(),
                r.bits_read()
            ));
        }
        block_no += 1;
        if bfinal == 1 {
            finished = true;
        }
    }
    Ok(out)
}

/// Decode a zlib container (RFC 1950): verify the header, decode, and
/// check the big-endian Adler-32 trailer.
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    if data.len() < 6 {
        return Err(InflateError::Truncated);
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0F != 8 {
        return Err(InflateError::BadFormat);
    }
    if (cmf as u16 * 256 + flg as u16) % 31 != 0 {
        return Err(InflateError::BadFormat);
    }
    if flg & 0x20 != 0 {
        // FDICT: preset dictionary — not supported (this stream family
        // never uses one).
        return Err(InflateError::BadFormat);
    }
    let out = decompress_raw(&data[2..data.len() - 4])?;
    let stored = u32::from_be_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    if crate::adler::adler32_fresh(&out) != stored {
        return Err(InflateError::ChecksumMismatch);
    }
    Ok(out)
}
