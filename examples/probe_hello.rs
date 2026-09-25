fn main() {
    let z = bitfold::deflate::compress_zlib(b"hello");
    println!("{}", z.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let (crc, adler) = bitfold::deflate::container_hashes(b"hello");
    println!("crc {crc:#010x} adler {adler:#010x}");
    // pinned raw blocks (bfinal=1: each is a complete single-block stream)
    let stored = [0x01u8, 0x03, 0x00, 0xfc, 0xff, 0x41, 0x42, 0x43];
    let back = bitfold::inflate::decompress_raw(&stored).unwrap();
    assert_eq!(back, b"ABC");
    let fixed = [0x4Bu8, 0x04, 0x00];
    let back = bitfold::inflate::decompress_raw(&fixed).unwrap();
    assert_eq!(back, [0x61]);
    println!("pinned raw blocks OK");
}
