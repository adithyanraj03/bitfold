fn main() {
    let mut msg: Vec<u8> = Vec::new();
    for _ in 0..50 {
        msg.extend_from_slice(b"The quick brown fox jumps over the lazy dog. ");
    }
    let z = bitfold::deflate::compress_zlib(&msg);
    println!("{} bytes", z.len());
    println!("{}", z.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let back = bitfold::inflate::decompress_zlib(&z).expect("round-trip");
    assert_eq!(back, msg);
    println!("self round-trip OK");
}
