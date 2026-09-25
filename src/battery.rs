//! The verification report — the single source of truth for the dossier,
//! the attestation, and the SVG story.
//!
//! `render_report()` is pure and deterministic: every number in it is
//! computed from the embedded corpus and the KAT battery (no wall clock,
//! no environment, no RNG), so two renders in any two processes are
//! byte-identical.
//!
//! Artifact hashing convention (avoids a report <-> dossier cycle): the
//! report's hash table covers the SVGs, which are rendered from the
//! entropy ledger alone and therefore do not depend on the report text.
//! The report's and the dossier's own hashes are recorded by
//! `bitfold attest` (rendering both twice, in-process) and by the CLI
//! when it writes them.

use crate::crypto::{sha256, to_hex};
use crate::entropy::{efficiency_fp, ledger, wasted_bits_fp, SCALE};

/// Render the full verification report (ASCII, fixed layout).
#[must_use]
pub fn render_report() -> String {
    let corpus = crate::corpus::assemble();
    let ld = ledger(&[
        ("rfc1951", crate::corpus::RFC1951),
        ("rfc8439", crate::corpus::RFC8439),
        ("b0000796", crate::corpus::OEIS_B000796),
    ]);

    // ---- round trips ---------------------------------------------------
    let (raw, stats) = crate::deflate::compress_raw(&corpus);
    let raw_rt = crate::inflate::decompress_raw(&raw).map(|v| v == corpus).unwrap_or(false);
    let z = crate::deflate::compress_zlib(&corpus);
    let z_rt = crate::inflate::decompress_zlib(&z).map(|v| v == corpus).unwrap_or(false);
    let fox = fox_msg();
    let fz = crate::deflate::compress_zlib(&fox);
    let fz_rt = crate::inflate::decompress_zlib(&fz).map(|v| v == fox).unwrap_or(false);
    let (crc, adler) = crate::deflate::container_hashes(&corpus);

    // ---- KAT battery -----------------------------------------------------
    let checks = crate::kats::run_all();
    let pass = checks.iter().filter(|c| c.ok).count();

    let mut s = format_report_body(
        &corpus,
        &ld,
        raw.len(),
        raw_rt,
        z_rt,
        fz_rt,
        &z,
        &fz,
        crc,
        adler,
        &stats,
        pass,
        checks.len(),
        &checks,
    );

    // ---- artifact hash table (SVGs only — see module docs) --------------
    let anim = crate::svg::render_anim(6, &ld);
    let sheet = crate::svg::render_contact_sheet(3, &ld);
    s.push_str("\nartifacts (sha256 of the shipped renders):\n");
    s.push_str(&format!(
        "  {:<14} {:>6} B  {}\n",
        "anim.svg",
        anim.len(),
        to_hex(&sha256(anim.as_bytes()))
    ));
    s.push_str(&format!(
        "  {:<14} {:>6} B  {}\n",
        "frames.svg",
        sheet.len(),
        to_hex(&sha256(sheet.as_bytes()))
    ));
    s.push_str("  (this report's and the dossier's own hashes are pinned by\n");
    s.push_str("   `bitfold attest` — see attestation.txt.)\n");
    s
}

fn format_report_body(
    corpus: &[u8],
    ld: &crate::entropy::Ledger,
    raw_len: usize,
    raw_rt: bool,
    z_rt: bool,
    fz_rt: bool,
    z: &[u8],
    fz: &[u8],
    crc: u32,
    adler: u32,
    stats: &crate::deflate::CompressStats,
    pass: usize,
    total: usize,
    checks: &[crate::kats::Check],
) -> String {
    let mut s = String::new();
    let line = "=".repeat(72);
    s.push_str(&format!("{line}\nBITFOLD VERIFICATION REPORT\n"));
    s.push_str("RFC 1951 DEFLATE + CRC-32/Adler-32 — from scratch, std-only, verified\n");
    s.push_str(&format!("{line}\n"));
    s.push_str(&format!("version       : bitfold {}\n", crate::VERSION));

    // corpus
    s.push_str("\ncorpus (public-domain, embedded at compile time):\n");
    for p in &ld.parts {
        s.push_str(&format!(
            "  {:<10} {:>9} bytes  {:>4} distinct symbols\n",
            p.name, p.n_bytes, p.n_symbols
        ));
    }
    s.push_str(&format!("  {:<10} {:>9} bytes\n", "total", ld.n_bytes));

    // round trips
    s.push_str("\nround trips (encode -> decode, byte-identical):\n");
    s.push_str(&format!(
        "  raw DEFLATE corpus : {:>9} B in -> {:>8} B out  {}\n",
        corpus.len(),
        raw_len,
        mark(raw_rt)
    ));
    s.push_str(&format!(
        "  zlib container     : {:>9} B in -> {:>8} B out  {}  (78 9c + Adler-32)\n",
        corpus.len(),
        z.len(),
        mark(z_rt)
    ));
    s.push_str(&format!(
        "  fox message (2250) : {:>9} B in -> {:>8} B out  {}\n",
        2250usize,
        fz.len(),
        mark(fz_rt)
    ));
    s.push_str(&format!(
        "  container hashes   : crc32 = {crc:08x}  adler32 = {adler:08x}\n",
    ));

    // KAT battery
    s.push_str("\nKAT battery (deterministic known-answer checks):\n");
    for (label, (t, ok)) in group_totals(checks) {
        s.push_str(&format!("  {:<10} {:>4} checks, {:>4} pass\n", label, t, ok));
    }
    s.push_str(&format!(
        "  {:<10} {:>4} checks, {:>4} pass  {}\n",
        "total",
        total,
        pass,
        if pass == total { "ALL PASS" } else { "FAILURES BELOW" }
    ));
    for c in checks.iter().filter(|c| !c.ok) {
        s.push_str(&format!("  FAIL {} : {}\n", c.name, c.detail));
    }

    // entropy ledger
    s.push_str("\nentropy ledger (u128 fixed-point, 2^40 scale):\n");
    for p in &ld.parts {
        s.push_str(&format!(
            "  {:<10} H = {:>8.5} bits/byte  ratio bound {:>7.4}  top: 0x{:02x} x{}\n",
            p.name,
            p.entropy_bits_per_byte,
            p.ratio_bound,
            p.top[0].0,
            p.top[0].1
        ));
    }
    s.push_str(&format!(
        "  {:<10} H = {:>8.5} bits/byte  ratio bound {:>7.4}\n",
        "total", ld.entropy_bits_per_byte, ld.ratio_bound
    ));

    // encoder profile + fold
    //
    // The byte-histogram entropy is the bound for a *memoryless* encoder
    // (one code per byte). DEFLATE encodes LZ77 phrases, so the LZ77
    // stage may lawfully beat that bound by exploiting sequential
    // redundancy; `wasted_bits_fp` (above-bound slack) is therefore
    // expected to clamp to 0, and the interesting number is how far
    // *below* the bound the encoder lands.
    let bits = stats.deflate_bits;
    let wasted = wasted_bits_fp(bits, ld);
    let eff = efficiency_fp(bits, ld);
    let bound_bits_fp = ld.entropy_bits_fp;
    // How far *below* the memoryless bound the encoder lands (0 when at
    // or above it — i.e. when wasted_bits_fp > 0).
    let below_bits_fp = bound_bits_fp.saturating_sub(bits as u128 * SCALE);
    s.push_str("\nencoder profile (full corpus, one pass):\n");
    s.push_str(&format!(
        "  blocks         : {} total ({} stored, {} fixed, {} dynamic)\n",
        stats.n_blocks, stats.n_stored, stats.n_fixed, stats.n_dynamic
    ));
    s.push_str(&format!(
        "  phrases        : {} literals, {} matches ({} bytes copied)\n",
        stats.n_lit, stats.n_match, stats.match_bytes
    ));
    s.push_str(&format!(
        "  deflate bits   : {} logical bits ({} bytes on the wire)\n",
        bits,
        z.len() - 6
    ));
    s.push_str(&format!(
        "  achieved ratio : {:.4} ({} B / {} B)\n",
        z.len() as f64 / corpus.len() as f64,
        z.len(),
        corpus.len()
    ));
    if wasted > 0 {
        s.push_str(&format!(
            "  entropy fold   : {} bits above the memoryless bound ({:.4} bits/byte wasted)\n",
            wasted,
            wasted as f64 / SCALE as f64 / ld.n_bytes as f64
        ));
    } else {
        s.push_str(&format!(
            "  entropy fold   : {below:.0} bits BELOW the memoryless bound\n",
            below = below_bits_fp as f64 / SCALE as f64
        ));
        s.push_str(&format!(
            "                   ({:.4} bits/byte under H = {:.5}) — the LZ77 stage\n",
            below_bits_fp as f64 / SCALE as f64 / ld.n_bytes as f64,
            ld.entropy_bits_per_byte
        ));
        s.push_str("                   captured the sequential redundancy that per-byte\n");
        s.push_str("                   entropy cannot see; beating that bound is the win.\n");
    }
    s.push_str(&format!(
        "  efficiency     : {:.2}% of the memoryless bound (>100% = below it)\n",
        eff as f64 / SCALE as f64 * 100.0
    ));
    s.push_str("  lz77           : window 32768, min match 3, max match 258,\n");
    s.push_str(&format!(
        "                   chain limit {}, hash table 2^16 (chained hash)\n",
        crate::lzw77::MAX_CHAIN
    ));

    // cross-verification
    s.push_str("\ncross-verification (CPython zlib, pinned at development time):\n");
    for c in checks.iter().filter(|c| c.name.starts_with("xval-")) {
        s.push_str(&format!(
            "  {}  {}  {}\n",
            c.name,
            if c.ok { "PASS" } else { "FAIL" },
            c.detail
        ));
    }
    s.push_str("  encoder direction: our dynamic stream decodes byte-identical in\n");
    s.push_str("  zlib.decompress; decoder direction: zlib's fixed-block streams\n");
    s.push_str("  decode byte-identical here. The hexes are pinned in the KAT battery.\n");

    s
}

/// Per-group (total, pass) counts, in report order.
pub fn group_totals(checks: &[crate::kats::Check]) -> Vec<(&'static str, (usize, usize))> {
    let groups: [(&str, &[&str]); 5] = [
        ("rfc1951", &["rfc-"]),
        ("bits", &["bit-"]),
        ("blocks", &["stored-", "fixed-", "overlap-", "zlib-"]),
        ("hash", &["hash-"]),
        ("xval", &["xval-"]),
    ];
    let mut out = Vec::new();
    for (label, prefixes) in groups {
        let mut total = 0usize;
        let mut ok = 0usize;
        for c in checks {
            if prefixes.iter().any(|p| c.name.starts_with(p)) {
                total += 1;
                if c.ok {
                    ok += 1;
                }
            }
        }
        if total > 0 {
            out.push((label, (total, ok)));
        }
    }
    out
}

fn mark(ok: bool) -> &'static str {
    if ok {
        "byte-exact"
    } else {
        "MISMATCH"
    }
}

/// The 2250-byte cross-verification message (50 x 45).
fn fox_msg() -> Vec<u8> {
    let mut v = Vec::with_capacity(2250);
    for _ in 0..50 {
        v.extend_from_slice(b"The quick brown fox jumps over the lazy dog. ");
    }
    v
}
