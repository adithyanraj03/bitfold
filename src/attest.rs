//! Determinism attestation.
//!
//! The attestation renders the report and the dossier **twice, in
//! process**, and compares their SHA-256 digests. Both renders are pure
//! functions of the embedded corpus and the KAT battery, so a mismatch
//! means non-determinism in the build itself (or a compiler bug) — the
//! CLI exits non-zero on mismatch. The digests below pin the artifacts
//! to the committed sources: a cross-process re-run must reproduce the
//! same digests byte-for-byte.

use crate::crypto::{sha256, to_hex};

/// One attestation: two in-process renders of report and dossier.
#[derive(Debug, Clone)]
pub struct Attestation {
    pub version: &'static str,
    pub report_sha256_run1: [u8; 32],
    pub report_sha256_run2: [u8; 32],
    pub dossier_sha256_run1: [u8; 32],
    pub dossier_sha256_run2: [u8; 32],
    pub report_len: usize,
    pub dossier_len: usize,
    pub identical: bool,
}

/// Render report + dossier twice and compare.
#[must_use]
pub fn attest() -> Attestation {
    let (r1, d1) = render_pair();
    let (r2, d2) = render_pair();
    let a1 = sha256(r1.as_bytes());
    let a2 = sha256(r2.as_bytes());
    let b1 = sha256(&d1);
    let b2 = sha256(&d2);
    Attestation {
        version: crate::VERSION,
        report_sha256_run1: a1,
        report_sha256_run2: a2,
        dossier_sha256_run1: b1,
        dossier_sha256_run2: b2,
        report_len: r1.len(),
        dossier_len: d1.len(),
        identical: a1 == a2 && b1 == b2,
    }
}

fn render_pair() -> (String, Vec<u8>) {
    let report = crate::battery::render_report();
    let dossier = crate::pdf::render_dossier(&report);
    (report, dossier)
}

/// Render the attestation as its committed text form (fixed layout, no
/// wall clock — the date field is the pinned project date).
#[must_use]
pub fn render_attestation(a: &Attestation) -> String {
    let mut s = String::new();
    let line = "=".repeat(72);
    s.push_str(&line);
    s.push_str("\nBITFOLD DETERMINISM ATTESTATION\n");
    s.push_str("dual in-process render, SHA-256 comparison, fixed project date\n");
    s.push_str(&format!("{line}\n"));
    s.push_str(&format!("version       : bitfold {}\n", a.version));
    s.push_str(&format!("attested date : 2026-01-01 (pinned; no wall clock)\n"));
    s.push_str(&format!(
        "corpus        : {} bytes (3 public-domain parts, embedded)\n",
        crate::corpus::TOTAL_BYTES
    ));
    s.push_str(&format!(
        "KAT battery   : {} checks (13 rfc + 4 bits + 5 blocks + {} hashes + 4 xval)\n",
        crate::kats::check_count(),
        crate::kat_data::HASH_KATS.len()
    ));
    s.push_str("\nrender 1 (in-process):\n");
    s.push_str(&format!(
        "  report.txt      {:>7} B  {}\n",
        a.report_len,
        to_hex(&a.report_sha256_run1)
    ));
    s.push_str(&format!(
        "  dossier.pdf     {:>7} B  {}\n",
        a.dossier_len,
        to_hex(&a.dossier_sha256_run1)
    ));
    s.push_str("\nrender 2 (in-process):\n");
    s.push_str(&format!(
        "  report.txt      {:>7} B  {}\n",
        a.report_len,
        to_hex(&a.report_sha256_run2)
    ));
    s.push_str(&format!(
        "  dossier.pdf     {:>7} B  {}\n",
        a.dossier_len,
        to_hex(&a.dossier_sha256_run2)
    ));
    s.push_str("\nverdict       : ");
    if a.identical {
        s.push_str("BYTE-IDENTICAL — both renders reproduce the same digests.\n");
        s.push_str("A cross-process re-run of `bitfold attest` must reproduce these\n");
        s.push_str("four digests exactly; any difference breaks the determinism brand.\n");
    } else {
        s.push_str("NON-DETERMINISTIC — renders diverge; do not ship.\n");
    }
    s.push_str("\n(c) 2026 Adithya N Raj — bitfold, RFC 1951 from scratch, std-only.\n");
    s.push_str(&line);
    s.push('\n');
    s
}
