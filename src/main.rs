//! `bitfold` CLI — verification commands.
//!
//! ```text
//! bitfold help              this text
//! bitfold version           version line
//! bitfold demo              short demo (compress + fold ledger)
//! bitfold kats              KAT battery summary (RFC 1951 + zlib hashes)
//! bitfold battery [PATH]    write the verification report (default: assets/report.txt)
//! bitfold dossier [PATH]    write the PDF dossier (default: assets/dossier.pdf)
//! bitfold attest [PATH]     write the attestation (default: assets/attestation.txt)
//! bitfold svg [DIR]         write anim.svg + frames.svg (default: assets)
//! ```

use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let out = args.get(1).map(|s| s.as_str());
    let code = match cmd {
        "help" | "-h" | "--help" => {
            print_help();
            0
        }
        "version" | "-V" | "--version" => {
            println!("bitfold {}", bitfold::VERSION);
            0
        }
        "demo" => demo(),
        "kats" => kats(),
        "battery" => {
            let path = out.unwrap_or("assets/report.txt");
            let report = bitfold::battery::render_report();
            write_file(path, report.as_bytes());
            println!("wrote {path} ({} bytes)", report.len());
            0
        }
        "dossier" => {
            let path = out.unwrap_or("assets/dossier.pdf");
            let report = bitfold::battery::render_report();
            let pdf = bitfold::pdf::render_dossier(&report);
            write_file(path, &pdf);
            println!(
                "wrote {path} ({} bytes, sha256 {})",
                pdf.len(),
                bitfold::crypto::to_hex(&bitfold::crypto::sha256(&pdf))
            );
            0
        }
        "attest" => {
            let path = out.unwrap_or("assets/attestation.txt");
            let a = bitfold::attest::attest();
            let text = bitfold::attest::render_attestation(&a);
            write_file(path, text.as_bytes());
            print!("{text}");
            if a.identical {
                0
            } else {
                1
            }
        }
        "svg" => {
            let dir = out.unwrap_or("assets");
            let ld = bitfold::entropy::ledger(&[
                ("rfc1951", bitfold::corpus::RFC1951),
                ("rfc8439", bitfold::corpus::RFC8439),
                ("b0000796", bitfold::corpus::OEIS_B000796),
            ]);
            let anim = bitfold::svg::render_anim(6, &ld);
            let sheet = bitfold::svg::render_contact_sheet(3, &ld);
            let p1 = format!("{dir}/anim.svg");
            let p2 = format!("{dir}/frames.svg");
            write_file(&p1, anim.as_bytes());
            write_file(&p2, sheet.as_bytes());
            println!("wrote {p1} ({} bytes)", anim.len());
            println!("wrote {p2} ({} bytes)", sheet.len());
            0
        }
        other => {
            eprintln!("unknown command: {other}\n");
            print_help();
            2
        }
    };
    std::process::exit(code);
}

fn print_help() {
    println!(
        "bitfold {} - RFC 1951 DEFLATE + CRC-32/Adler-32, entropy-folded and verified",
        bitfold::VERSION
    );
    println!();
    println!("commands:");
    println!("  bitfold help              this text");
    println!("  bitfold version           version line");
    println!("  bitfold demo              short demo (compress + fold ledger)");
    println!("  bitfold kats              KAT battery summary (RFC 1951 + zlib hashes)");
    println!("  bitfold battery [PATH]    write the verification report (assets/report.txt)");
    println!("  bitfold dossier [PATH]    write the PDF dossier (assets/dossier.pdf)");
    println!("  bitfold attest [PATH]     write the attestation (assets/attestation.txt)");
    println!("  bitfold svg [DIR]         write anim.svg + frames.svg (assets)");
}

fn write_file(path: &str, bytes: &[u8]) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let mut f = std::fs::File::create(path).expect("create output file");
    f.write_all(bytes).expect("write output file");
}

fn demo() -> i32 {
    use std::fmt::Write;
    let msg = b"DEFLATE is the format; this demo folds a small message and checks the round trip.";
    let compressed = bitfold::deflate::compress_zlib(msg);
    let recovered = bitfold::inflate::decompress_zlib(&compressed).unwrap_or_default();
    let round_ok = recovered == msg;

    // A bigger demo: the corpus, one block budget, full ledger.
    let corpus = bitfold::corpus::assemble();
    let big = bitfold::deflate::compress_zlib(&corpus);
    let big_ok = bitfold::inflate::decompress_zlib(&big).map(|v| v == corpus).unwrap_or(false);
    let (crc, adler) = bitfold::deflate::container_hashes(&corpus);

    let mut s = String::new();
    let _ = writeln!(s, "deflate demo — zlib container (0x78 0x9c + Adler-32)");
    let _ = writeln!(s, "  message        : {} bytes", msg.len());
    let _ = writeln!(s, "  compressed     : {} bytes (ratio {:.4})", compressed.len(), compressed.len() as f64 / msg.len() as f64);
    let _ = writeln!(s, "  round trip     : {}", if round_ok { "byte-exact" } else { "MISMATCH" });
    let _ = writeln!(s, "");
    let _ = writeln!(s, "corpus (3 public-domain parts, {} bytes):", corpus.len());
    let ledger = bitfold::entropy::ledger(&[
        ("rfc1951", bitfold::corpus::RFC1951),
        ("rfc8439", bitfold::corpus::RFC8439),
        ("b0000796", bitfold::corpus::OEIS_B000796),
    ]);
    for p in &ledger.parts {
        let _ = writeln!(
            s,
            "  {:<10} {:>9} bytes  H = {:>8.5} bits/byte  ratio bound {:>7.4}",
            p.name, p.n_bytes, p.entropy_bits_per_byte, 1.0 / p.entropy_bits_per_byte * 8.0
        );
    }
    let _ = writeln!(
        s,
        "  {:<10} {:>9} bytes  H = {:>8.5} bits/byte  ratio bound {:>7.4}",
        "total", ledger.n_bytes, ledger.entropy_bits_per_byte,
        1.0 / ledger.entropy_bits_per_byte * 8.0
    );
    let _ = writeln!(
        s,
        "  compressed     : {} bytes (ratio {:.4} of {})",
        big.len(),
        big.len() as f64 / corpus.len() as f64,
        corpus.len()
    );
    let _ = writeln!(s, "  round trip     : {}", if big_ok { "byte-exact" } else { "MISMATCH" });
    let _ = writeln!(
        s,
        "  crc32 = {}  adler32 = {}",
        bitfold::crypto::to_hex(&crc.to_le_bytes()),
        bitfold::crypto::to_hex(&adler.to_le_bytes())
    );
    println!("{s}");
    if round_ok && big_ok {
        0
    } else {
        1
    }
}

fn kats() -> i32 {
    let checks = bitfold::kats::run_all();
    let mut ok = true;
    for (i, c) in checks.iter().enumerate() {
        if !c.ok {
            ok = false;
        }
        println!(
            "[{:>4}] {:<46} {}",
            if c.ok { "PASS" } else { "FAIL" },
            format!("#{} {}", i + 1, c.name),
            c.detail
        );
    }
    let pass = checks.iter().filter(|c| c.ok).count();
    println!();
    println!(
        "summary: {} checks, {} pass, {} fail  ->  {}",
        checks.len(),
        pass,
        checks.len() - pass,
        if ok { "ALL KATS PASS" } else { "KAT FAILURES" }
    );
    if ok {
        0
    } else {
        1
    }
}
