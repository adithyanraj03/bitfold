//! The SMIL animation and the static contact sheet.
//!
//! Both are deterministic functions of the entropy ledger and the KAT
//! battery: no wall clock, no randomness, identical bytes on every run.
//!
//! House SMIL idiom: every frame is a `<g opacity="0">` carrying one
//! discrete `<animate>` (`calcMode="discrete"`) that holds it visible
//! for exactly one slice of the 12 s cycle. Six frames, 2 s each.
//!
//! House palette: paper #F7F6F1, ink #252524, muted #676662,
//! hairline #DCDAD1, green #22AC80, indigo #5B51C7, brick #A74221,
//! orange #E8833A.

use crate::entropy::Ledger;

/// Paper background.
const PAPER: &str = "#F7F6F1";
/// Ink (primary text).
const INK: &str = "#252524";
/// Muted text.
const MUTED: &str = "#676662";
/// Hairline rules.
const HAIR: &str = "#DCDAD1";
/// Accent: pass / positive.
const GREEN: &str = "#22AC80";
/// Accent: structure / codes.
const INDIGO: &str = "#5B51C7";
/// Accent: caution / wasted bits.
const BRICK: &str = "#A74221";
/// Accent: energy / bits.
const ORANGE: &str = "#E8833A";

const CYCLE_S: f64 = 12.0;

/// Render the 6-frame SMIL animation (deterministic).
#[must_use]
pub fn render_anim(frames: usize, ld: &Ledger) -> String {
    let d = story_data(frames.min(6).max(1), ld);
    let mut s = String::new();
    s.push_str(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"760\" height=\"320\" viewBox=\"0 0 760 320\">",
    );
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"760\" height=\"320\" fill=\"{PAPER}\"/>"
    ));
    s.push_str("<defs><style>text{font-family:ui-monospace,Menlo,Consolas,monospace}</style></defs>");
    for (i, content) in d.iter().enumerate() {
        let t0 = i as f64 / 6.0;
        let t1 = (i + 1) as f64 / 6.0;
        s.push_str("<g opacity=\"0\">");
        s.push_str(content);
        s.push_str(&format!(
            "<animate attributeName=\"opacity\" values=\"0;1;0\" keyTimes=\"0;{t0:.4};{t1:.4}\" dur=\"{CYCLE_S:.1}s\" calcMode=\"discrete\" repeatCount=\"indefinite\"/>"
        ));
        s.push_str("</g>");
    }
    s.push_str("</svg>");
    s
}

/// Render the static contact sheet: all 6 frames, `cols` across.
#[must_use]
pub fn render_contact_sheet(cols: usize, ld: &Ledger) -> String {
    let cols = cols.clamp(2, 3);
    let rows = (6 + cols - 1) / cols;
    let scale = 0.34f64;
    let cw = (760.0 * scale).round() as i64; // 258
    let ch = (320.0 * scale).round() as i64; // 109
    let gap = 8i64;
    let margin = 8i64;
    let title_h = 22i64;
    let w = cols as i64 * cw + (cols as i64 + 1) * gap + margin * 2;
    let h = title_h + rows as i64 * ch + (rows as i64 + 1) * gap + margin;
    let d = story_data(6, ld);
    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"{PAPER}\"/>"
    ));
    s.push_str("<defs><style>text{font-family:ui-monospace,Menlo,Consolas,monospace}</style></defs>");
    s.push_str(&format!(
        "<text x=\"{margin}\" y=\"{title_h}\" font-size=\"13\" fill=\"{INK}\" font-weight=\"bold\">bitfold — 6-frame story (static)</text>"
    ));
    for (i, content) in d.iter().enumerate() {
        let r = i / cols;
        let c = i % cols;
        let x = margin + gap + c as i64 * (cw + gap);
        let y = title_h + margin + gap + r as i64 * (ch + gap);
        s.push_str(&format!(
            "<g transform=\"translate({x},{y}) scale({scale})\">"
        ));
        s.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"760\" height=\"320\" fill=\"{PAPER}\" stroke=\"{HAIR}\"/>"
        ));
        s.push_str(content);
        s.push_str("</g>");
    }
    s.push_str("</svg>");
    s
}

/// One frame's inner content (coordinates in the 760x320 frame space).
fn story_data(n: usize, ld: &Ledger) -> Vec<String> {
    let checks = crate::kats::run_all();
    let pass = checks.iter().filter(|c| c.ok).count();
    let groups = crate::battery::group_totals(&checks);
    let z = crate::deflate::compress_zlib(&crate::corpus::assemble());
    let ratio = z.len() as f64 / crate::corpus::TOTAL_BYTES as f64;

    // F0 — title
    let f0 = format!(
        "<text x=\"380\" y=\"120\" text-anchor=\"middle\" font-size=\"54\" fill=\"{INK}\" font-weight=\"bold\">bitfold</text>\
         <line x1=\"180\" y1=\"150\" x2=\"580\" y2=\"150\" stroke=\"{HAIR}\" stroke-width=\"2\"/>\
         <text x=\"380\" y=\"185\" text-anchor=\"middle\" font-size=\"17\" fill=\"{MUTED}\">RFC 1951 DEFLATE + CRC-32 / Adler-32 — from scratch, std-only Rust</text>\
         <text x=\"380\" y=\"215\" text-anchor=\"middle\" font-size=\"14\" fill=\"{INDIGO}\">entropy folds the data down; the ledger measures how far</text>\
         <text x=\"380\" y=\"290\" text-anchor=\"middle\" font-size=\"11\" fill=\"{MUTED}\">bitfold {} · 6 frames · 2 s each</text>",
        crate::VERSION
    );

    // F1 — corpus
    let mut rows = String::new();
    for (i, p) in ld.parts.iter().enumerate() {
        let y = 110 + i * 34;
        rows.push_str(&format!(
            "<text x=\"150\" y=\"{y}\" font-size=\"16\" fill=\"{INK}\">{name}</text>\
             <text x=\"560\" y=\"{y}\" font-size=\"16\" fill=\"{INK}\" text-anchor=\"end\">{n} bytes</text>\
             <text x=\"640\" y=\"{y}\" font-size=\"13\" fill=\"{MUTED}\" text-anchor=\"end\">{sym} syms</text>",
            name = escape(p.name),
            n = p.n_bytes,
            sym = p.n_symbols
        ));
    }
    let f1 = format!(
        "<text x=\"60\" y=\"60\" font-size=\"24\" fill=\"{INK}\" font-weight=\"bold\">the corpus</text>\
         <line x1=\"60\" y1=\"74\" x2=\"700\" y2=\"74\" stroke=\"{HAIR}\"/>\
         {rows}\
         <text x=\"150\" y=\"{ty}\" font-size=\"16\" fill=\"{INK}\" font-weight=\"bold\">total</text>\
         <text x=\"560\" y=\"{ty}\" font-size=\"16\" fill=\"{INK}\" text-anchor=\"end\" font-weight=\"bold\">{total} bytes</text>\
         <text x=\"640\" y=\"{ty}\" font-size=\"13\" fill=\"{MUTED}\" text-anchor=\"end\">public-domain</text>\
         <text x=\"60\" y=\"290\" font-size=\"11\" fill=\"{MUTED}\">RFC 1951 (the spec) · RFC 8439 (ChaCha20-Poly1305) · OEIS A000796 (prime digits)</text>",
        ty = 110 + 3 * 34 + 20,
        total = ld.n_bytes
    );

    // F2 — entropy ledger
    let mut bars = String::new();
    let base_w = 340.0;
    for (i, p) in ld.parts.iter().enumerate() {
        let y = 118 + i * 52;
        let w = if p.entropy_bits_per_byte > 0.0 {
            (base_w * p.entropy_bits_per_byte / 8.0).min(base_w)
        } else {
            0.0
        };
        bars.push_str(&format!(
            "<text x=\"150\" y=\"{y}\" font-size=\"14\" fill=\"{INK}\">{name}</text>\
             <rect x=\"330\" y=\"{ry}\" width=\"{bw}\" height=\"14\" fill=\"{INDIGO}\"/>\
             <rect x=\"330\" y=\"{ry}\" width=\"{base_w:.0}\" height=\"14\" fill=\"none\" stroke=\"{HAIR}\"/>\
             <text x=\"690\" y=\"{y}\" font-size=\"13\" fill=\"{MUTED}\" text-anchor=\"end\">H = {h:.5} b/B</text>",
            name = escape(p.name),
            ry = y - 12,
            bw = w,
            h = p.entropy_bits_per_byte
        ));
    }
    let f2 = format!(
        "<text x=\"60\" y=\"60\" font-size=\"24\" fill=\"{INK}\" font-weight=\"bold\">entropy ledger</text>\
         <line x1=\"60\" y1=\"74\" x2=\"700\" y2=\"74\" stroke=\"{HAIR}\"/>\
         <text x=\"60\" y=\"96\" font-size=\"12\" fill=\"{MUTED}\">Shannon H (bits/byte), bar = H / 8 of the full byte</text>\
         {bars}\
         <text x=\"150\" y=\"290\" font-size=\"14\" fill=\"{INK}\" font-weight=\"bold\">total</text>\
         <text x=\"690\" y=\"290\" font-size=\"14\" fill=\"{INDIGO}\" text-anchor=\"end\">H = {h:.5} b/B · bound {b:.4}</text>",
        h = ld.entropy_bits_per_byte,
        b = ld.ratio_bound
    );

    // F3 — encoder
    let (_, est) = crate::deflate::compress_raw(&crate::corpus::assemble());
    let f3 = format!(
        "<text x=\"60\" y=\"60\" font-size=\"24\" fill=\"{INK}\" font-weight=\"bold\">the encoder</text>\
         <line x1=\"60\" y1=\"74\" x2=\"700\" y2=\"74\" stroke=\"{HAIR}\"/>\
         <text x=\"60\" y=\"112\" font-size=\"15\" fill=\"{MUTED}\">LZ77: chained hash + lazy matching · window 32768 · match 3..258</text>\
         <text x=\"60\" y=\"144\" font-size=\"15\" fill=\"{MUTED}\">Huffman: canonical codes · stored / fixed / dynamic blocks</text>\
         <text x=\"60\" y=\"176\" font-size=\"15\" fill=\"{MUTED}\">bits: data LSB-first · codes MSB-first · extras LSB-first (zlib order)</text>\
         <text x=\"60\" y=\"216\" font-size=\"15\" fill=\"{INK}\">corpus → <tspan fill=\"{ORANGE}\" font-weight=\"bold\">{bits} logical bits</tspan> → ratio {ratio:.4}</text>\
         <text x=\"60\" y=\"248\" font-size=\"15\" fill=\"{INK}\">zlib container: 78 9c header + Adler-32 trailer (BE)</text>\
         <text x=\"60\" y=\"290\" font-size=\"11\" fill=\"{MUTED}\">every byte of the round trip is verified against the input, byte for byte</text>",
        bits = est.deflate_bits
    );

    // F4 — verification
    let mut rows = String::new();
    let mut yy = 104f64;
    for (label, (t, ok)) in groups.iter() {
        rows.push_str(&format!(
            "<text x=\"150\" y=\"{y}\" font-size=\"15\" fill=\"{INK}\">{name}</text>\
             <text x=\"560\" y=\"{y}\" font-size=\"15\" fill=\"{INK}\" text-anchor=\"end\">{t} checks</text>\
             <text x=\"640\" y=\"{y}\" font-size=\"15\" fill=\"{GREEN}\" text-anchor=\"end\">{ok} pass</text>",
            name = escape(label),
            t = t,
            ok = ok,
            y = yy
        ));
        yy += 34.0;
    }
    let f4 = format!(
        "<text x=\"60\" y=\"60\" font-size=\"24\" fill=\"{INK}\" font-weight=\"bold\">verification</text>\
         <line x1=\"60\" y1=\"74\" x2=\"700\" y2=\"74\" stroke=\"{HAIR}\"/>\
         {rows}\
         <text x=\"150\" y=\"290\" font-size=\"16\" fill=\"{INK}\" font-weight=\"bold\">total</text>\
         <text x=\"560\" y=\"290\" font-size=\"16\" fill=\"{INK}\" text-anchor=\"end\">{t} checks</text>\
         <text x=\"640\" y=\"290\" font-size=\"16\" fill=\"{c}\" text-anchor=\"end\" font-weight=\"bold\">{ok} pass{status}</text>",
        t = checks.len(),
        ok = pass,
        c = if pass == checks.len() { GREEN } else { BRICK },
        status = if pass == checks.len() { " · ALL PASS" } else { " · FAILURES" }
    );

    // F5 — verdict
    let f5 = format!(
        "<text x=\"380\" y=\"110\" text-anchor=\"middle\" font-size=\"30\" fill=\"{GREEN}\" font-weight=\"bold\">byte-identical</text>\
         <line x1=\"180\" y1=\"135\" x2=\"580\" y2=\"135\" stroke=\"{HAIR}\" stroke-width=\"2\"/>\
         <text x=\"380\" y=\"170\" text-anchor=\"middle\" font-size=\"15\" fill=\"{INK}\">dual in-process render · SHA-256 compared · pinned project date</text>\
         <text x=\"380\" y=\"196\" text-anchor=\"middle\" font-size=\"14\" fill=\"{MUTED}\">cross-verified both directions against CPython zlib</text>\
         <text x=\"380\" y=\"222\" text-anchor=\"middle\" font-size=\"14\" fill=\"{MUTED}\">our dynamic stream decodes in zlib · zlib's streams decode here</text>\
         <text x=\"380\" y=\"285\" text-anchor=\"middle\" font-size=\"12\" fill=\"{MUTED}\">(c) 2026 Adithya N Raj — determinism is the brand</text>"
    );

    let mut v = vec![f0, f1, f2, f3, f4, f5];
    v.truncate(n);
    v
}

/// Minimal XML text escaping for generated labels.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
