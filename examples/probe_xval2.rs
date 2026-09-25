fn main() {
    let msg = std::fs::read("target/fox_msg.bin").expect("fox_msg");
    let mut streams: Vec<(String, Vec<u8>)> = Vec::new();
    for line in std::fs::read_to_string("target/cpython_hex.txt").expect("hex file").lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut out = Vec::with_capacity(line.len() / 2);
        for i in (0..line.len()).step_by(2) {
            out.push(u8::from_str_radix(&line[i..i + 2], 16).unwrap());
        }
        streams.push((String::new(), out));
    }
    for (i, (name, z)) in streams.iter().enumerate() {
        let expected = if i < 3 {
            std::fs::read("target/fox_msg.bin").unwrap()
        } else {
            std::fs::read("target/data_msg.bin").unwrap()
        };
        let back = bitfold::inflate::decompress_zlib(z)
            .map_err(|e| format!("{e:?}"))
            .unwrap_or_else(|_| Vec::new());
        let ok = back == expected;
        println!(
            "stream {i} ({name}): {} bytes in, {} bytes out, match={ok}",
            z.len(),
            back.len()
        );
    }
    // Also: a stream with a STORED block. CPython zlib does not emit stored
    // blocks; build one by hand from our own primitives instead.
    let data: Vec<u8> = (0..2000u16).map(|i| (i % 251) as u8).collect();
    let (raw, _) = bitfold::deflate::compress_raw(&data);
    let z = bitfold::deflate::compress_zlib(&data);
    let back = bitfold::inflate::decompress_zlib(&z).unwrap();
    println!("self stored-capable stream: {} bytes, match={}", back.len(), back == data);
    let _ = raw;
}
