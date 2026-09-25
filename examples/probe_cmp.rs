fn main() {
    let corpus = bitfold::corpus::assemble();
    let (raw, stats) = bitfold::deflate::compress_raw(&corpus);
    println!(
        "blocks: {:?}",
        stats.blocks.iter().map(|b| (b.btype, b.n_bytes, b.bits)).collect::<Vec<_>>()
    );
    let mut log: Vec<String> = Vec::new();
    let back = match bitfold::inflate::decompress_raw_debug(&raw, &mut log) {
        Ok(v) => v,
        Err(e) => {
            for l in &log {
                println!("  {l}");
            }
            println!("ERR {e:?}");
            return;
        }
    };
    for l in &log {
        println!("{l}");
    }
    println!("in {} out {}", corpus.len(), back.len());
    if back.len() != corpus.len() {
        return;
    }
    for i in 0..corpus.len() {
        if corpus[i] != back[i] {
            let a = i.saturating_sub(16);
            println!(
                "first diff at {i}: expect {:02x} got {:02x} | ctx in  {:02x?}",
                corpus[i],
                back[i],
                &corpus[a..i.min(corpus.len())]
            );
            println!("                       got {:02x?}", &back[a..i]);
            // Show the next few bytes of each.
            let b = (i + 1).min(corpus.len());
            println!("                       in  next: {:02x?}", &corpus[b..(b + 16).min(corpus.len())]);
            println!("                       got next: {:02x?}", &back[b..(b + 16).min(corpus.len())]);
            return;
        }
    }
    println!("IDENTICAL");
}
