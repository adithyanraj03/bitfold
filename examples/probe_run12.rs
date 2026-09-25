fn main() {
    use bitfold::huff::{decode_code_lengths, encode_code_lengths};
    use bitfold::bits::BitError;
    let seq: Vec<u8> = vec![8; 12];
    let (syms, reps) = encode_code_lengths(&seq);
    println!("syms {syms:?} reps {reps:?}");
    struct St {
        syms: Vec<u32>,
        reps: Vec<u32>,
        i: usize,
    }
    let st = std::rc::Rc::new(std::cell::RefCell::new(St {
        syms: syms.clone(),
        reps: reps.clone(),
        i: 0,
    }));
    let st1 = st.clone();
    let st2 = st.clone();
    let r = decode_code_lengths(
        || {
            let mut g = st1.borrow_mut();
            if g.i >= g.syms.len() {
                eprintln!("read_sym: exhausted (i={})", g.i);
                return Err(BitError::Exhausted);
            }
            let s = g.syms[g.i];
            g.i += 1;
            eprintln!("read_sym -> {s} (i={})", g.i);
            Ok(s)
        },
        |_n| {
            let mut g = st2.borrow_mut();
            if g.i == 0 || g.i > g.reps.len() {
                eprintln!("read_bits: exhausted (i={})", g.i);
                return Err(BitError::Exhausted);
            }
            let r = g.reps[g.i - 1];
            g.i += 1;
            eprintln!("read_bits -> {r} (i={})", g.i);
            Ok(r as u64)
        },
        12,
    );
    println!("decode: {r:?}");
}
