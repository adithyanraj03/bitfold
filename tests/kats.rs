//! KAT battery: every check must pass, and the count must be pinned.

#[test]
fn full_battery_passes() {
    let checks = bitfold::kats::run_all();
    let failed: Vec<_> = checks.iter().filter(|c| !c.ok).collect();
    assert_eq!(
        failed.len(),
        0,
        "failed: {}",
        failed
            .iter()
            .take(8)
            .map(|c| format!("{}: {}", c.name, c.detail))
            .collect::<Vec<_>>()
            .join("; ")
    );
    eprintln!(
        "battery: {}/{} checks passed",
        checks.iter().filter(|c| c.ok).count(),
        checks.len()
    );
}

#[test]
fn battery_count_pinned() {
    let n = bitfold::kats::run_all().len();
    assert_eq!(n, bitfold::kats::check_count(), "dynamic vs static count");
    assert!(
        n >= 185,
        "battery shrank to {n}; the full battery is 13 + 4 + 5 + 162 + 4 = 188"
    );
}

#[test]
fn rfc_group_names_present() {
    let checks = bitfold::kats::run_all();
    let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
    for want in [
        "rfc-canonical-example",
        "rfc-run-example-8x12",
        "rfc-length-code-roundtrip",
        "rfc-distance-code-roundtrip",
        "bit-lsb-pattern",
        "bit-huff-pattern",
        "stored-block-pinned",
        "fixed-block-pinned",
        "overlap-match-kat",
        "zlib-container-hello",
        "zlib-headers-rejected",
        "hash-001",
        "hash-162",
        "xval-our-dynamic-fox",
        "xval-cpython-fixed-fox-1",
        "xval-cpython-fixed-fox-6",
        "xval-cpython-fixed-data",
    ] {
        assert!(names.iter().any(|n| *n == want), "missing {want}");
    }
}
