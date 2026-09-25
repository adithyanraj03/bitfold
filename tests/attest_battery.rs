//! Artifact tests: report/dossier/attestation/SVG determinism, structure,
//! and the house-format pins.

use bitfold::attest::{attest, render_attestation};
use bitfold::entropy::ledger;

fn corpus_parts() -> [(&'static str, &'static [u8]); 3] {
    [
        ("rfc1951", bitfold::corpus::RFC1951),
        ("rfc8439", bitfold::corpus::RFC8439),
        ("b0000796", bitfold::corpus::OEIS_B000796),
    ]
}

#[test]
fn report_is_deterministic() {
    let a = bitfold::battery::render_report();
    let b = bitfold::battery::render_report();
    assert_eq!(a, b);
    assert!(!a.is_empty());
}

#[test]
fn dossier_is_deterministic_and_pdf() {
    let r1 = bitfold::battery::render_report();
    let r2 = bitfold::battery::render_report();
    let d1 = bitfold::pdf::render_dossier(&r1);
    let d2 = bitfold::pdf::render_dossier(&r2);
    assert_eq!(d1, d2);
    assert!(d1.starts_with(b"%PDF-1."));
    // Two or more pages.
    let text = String::from_utf8_lossy(&d1);
    let pages = text.matches("/Type /Page").count();
    assert!(pages >= 2, "dossier has {pages} pages");
    // The report content must survive into the PDF text layer.
    assert!(text.contains("BITFOLD VERIFICATION"));
}

#[test]
fn attestation_is_byte_identical() {
    let a = attest();
    assert!(a.identical, "dual render diverged");
    assert_eq!(a.report_sha256_run1, a.report_sha256_run2);
    assert_eq!(a.dossier_sha256_run1, a.dossier_sha256_run2);
    assert_eq!(a.version, bitfold::VERSION);
}

#[test]
fn attestation_text_format() {
    let a = attest();
    let t = render_attestation(&a);
    assert!(t.contains("BITFOLD DETERMINISM ATTESTATION"));
    assert!(t.contains("2026-01-01"));
    assert!(t.contains("BYTE-IDENTICAL"));
    assert!(t.contains("(c) 2026 Adithya N Raj"));
    // All four 64-char hex digests present.
    let hex64 = |s: &str| {
        s.chars().filter(|c| c.is_ascii_hexdigit()).count() >= 64
    };
    let lines: Vec<&str> = t.lines().collect();
    let digest_lines = lines
        .iter()
        .filter(|l| l.contains("B  "))
        .count();
    assert_eq!(digest_lines, 4, "expected 4 digest lines, got {digest_lines}");
    assert!(hex64(&t));
}

#[test]
fn report_pins_corpus_and_battery() {
    let t = bitfold::battery::render_report();
    assert!(t.contains("274689"), "total corpus bytes pinned");
    assert!(t.contains("188 checks"), "KAT battery pinned");
    assert!(t.contains("ALL PASS"));
    assert!(t.contains("byte-exact"));
    assert!(t.contains("crc32 = "));
    assert!(t.contains("(c) 2026 Adithya N Raj") || t.contains("attest"));
}

#[test]
fn anim_svg_structure() {
    let ld = ledger(&corpus_parts());
    let a = bitfold::svg::render_anim(6, &ld);
    let b = bitfold::svg::render_anim(6, &ld);
    assert_eq!(a, b, "anim must be deterministic");
    assert!(a.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    // Six hidden frame groups, six discrete animations.
    let frames = a.matches("<g opacity=\"0\">").count();
    assert_eq!(frames, 6, "expected 6 frames, got {frames}");
    let anims = a.matches("repeatCount=\"indefinite\"").count();
    assert_eq!(anims, 6);
    assert!(a.contains("calcMode=\"discrete\""));
    assert!(a.contains("(c) 2026 Adithya N Raj"));
    assert!(a.contains("bitfold "));
}

#[test]
fn contact_sheet_structure() {
    let ld = ledger(&corpus_parts());
    let s = bitfold::svg::render_contact_sheet(3, &ld);
    assert!(s.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(s.contains("6-frame story (static)"));
    // Six scaled cells.
    assert_eq!(s.matches("scale(0.34)").count(), 6);
    // Static: no SMIL.
    assert!(!s.contains("<animate"));
}

#[test]
fn artifacts_hash_table_is_stable() {
    let a = bitfold::battery::render_report();
    let b = bitfold::battery::render_report();
    let seg = |t: &str| {
        t.split("artifacts (sha256 of the shipped renders):")
            .nth(1)
            .unwrap()
            .to_string()
    };
    assert_eq!(seg(&a), seg(&b));
    assert!(seg(&a).contains("anim.svg"));
    assert!(seg(&a).contains("frames.svg"));
}

#[test]
fn version_line() {
    assert_eq!(bitfold::VERSION, "1.0.0");
}
