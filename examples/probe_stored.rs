fn main() {
    let stored = [0x00u8, 0x03, 0x00, 0xfc, 0xff, 0x41, 0x42, 0x43];
    let mut log = Vec::new();
    let r = bitfold::inflate::decompress_raw_debug(&stored, &mut log);
    println!("{r:?}");
    for line in &log {
        println!("  {line}");
    }
    // Direct bit-level trace with our own reader.
    use bitfold::bits::BitReader;
    let mut r = BitReader::new(&stored);
    let bfinal = r.next_lsb(1).unwrap();
    let btype = r.next_lsb(2).unwrap();
    println!("bfinal {bfinal} btype {btype} bits_read {}", r.bits_read());
    r.align_to_byte();
    println!("after align: bit {}", r.bits_read());
    match r.next_lsb(16) {
        Ok(v) => println!("LEN {v:#06x} bits {}", r.bits_read()),
        Err(e) => println!("LEN ERR {e:?}"),
    }
    match r.next_lsb(16) {
        Ok(v) => println!("NLEN {v:#06x} bits {}", r.bits_read()),
        Err(e) => println!("NLEN ERR {e:?}"),
    }
}
