//! LSB-first bit packing (RFC 1951 §3.1.1).
//!
//! * Data elements are packed into bytes in order of **increasing bit
//!   number** within the byte — starting with the least-significant bit.
//! * Data elements other than Huffman codes are packed starting with the
//!   **least-significant bit** of the element.
//! * Huffman codes are packed starting with the **most-significant bit**
//!   of the code (the first code bit lands in the lowest unused bit
//!   position of the stream).

/// An error raised when the bit stream runs out of bits or is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    /// The stream ended in the middle of a requested element.
    Exhausted,
}

/// Writes bits into a growing byte vector, LSB-first within each byte.
///
/// `push_lsb` is for fixed-width elements (block headers, LEN/NLEN, extra
/// bits, repeat counts): the element's LSB goes into the lowest unused
/// stream position. `push_huff` is for Huffman codes: the code's MSB (its
/// first bit) goes into the lowest unused position.
#[derive(Debug, Default, Clone)]
pub struct BitWriter {
    acc: u64,
    nbits: usize,
    out: Vec<u8>,
}

impl BitWriter {
    /// An empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push `nbits` bits of `value`, LSB of `value` first.
    pub fn push_lsb(&mut self, value: u64, nbits: u32) {
        debug_assert!(nbits <= 64);
        if self.nbits + nbits as usize > 56 {
            self.flush_full_bytes();
        }
        let mask = if nbits == 64 { u64::MAX } else { (1u64 << nbits) - 1 };
        self.acc |= (value & mask) << self.nbits;
        self.nbits += nbits as usize;
    }

    /// Push a Huffman `code` of `nbits` bits, MSB of the code first
    /// (the first code bit takes the lowest unused stream position).
    pub fn push_huff(&mut self, code: u32, nbits: u32) {
        debug_assert!(nbits <= 32);
        if self.nbits + nbits as usize > 56 {
            self.flush_full_bytes();
        }
        for k in 0..nbits {
            let bit = (code >> (nbits - 1 - k)) & 1;
            self.acc |= (bit as u64) << self.nbits;
            self.nbits += 1;
        }
    }

    fn flush_full_bytes(&mut self) {
        while self.nbits >= 8 {
            self.out.push((self.acc & 0xff) as u8);
            self.acc >>= 8;
            self.nbits -= 8;
        }
    }

    /// Bytes already flushed (the partial tail byte is not included).
    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Pad to a byte boundary, flush, and return the finished byte stream.
    pub fn finish(mut self) -> Vec<u8> {
        if self.nbits % 8 != 0 {
            self.nbits += 8 - (self.nbits % 8);
        }
        self.flush_full_bytes();
        self.out
    }
}

/// Reads bits from a byte slice, LSB-first within each byte.
#[derive(Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u32,
    /// Total bits consumed (padding included).
    bits_read: usize,
}

impl<'a> BitReader<'a> {
    /// A reader over `data`, positioned before the first bit.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte: 0,
            bit: 0,
            bits_read: 0,
        }
    }

    /// Read one bit (the next lowest unused position).
    fn read_bit(&mut self) -> Result<u64, BitError> {
        if self.byte >= self.data.len() {
            return Err(BitError::Exhausted);
        }
        let b = (self.data[self.byte] >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.byte += 1;
        }
        self.bits_read += 1;
        Ok(b as u64)
    }

    /// Read `nbits` bits, LSB of the returned value first (fixed-width
    /// element in the stream's natural order).
    pub fn next_lsb(&mut self, nbits: u32) -> Result<u64, BitError> {
        let mut v = 0u64;
        for k in 0..nbits {
            v |= self.read_bit()? << k;
        }
        Ok(v)
    }

    /// Read a `nbits`-bit Huffman code: the first bit read is the code's
    /// most-significant bit.
    pub fn next_huff(&mut self, nbits: u32) -> Result<u32, BitError> {
        let mut v = 0u32;
        for _ in 0..nbits {
            v = (v << 1) | (self.read_bit()? as u32);
        }
        Ok(v)
    }

    /// Skip to the next byte boundary.
    pub fn align_to_byte(&mut self) {
        if self.bit != 0 {
            self.bits_read += 8 - self.bit as usize;
            self.bit = 0;
            self.byte += 1;
        }
    }

    /// Total bits consumed so far (padding included).
    pub fn bits_read(&self) -> usize {
        self.bits_read
    }

    /// True once the reader has passed the last byte.
    pub fn at_end(&self) -> bool {
        self.byte >= self.data.len()
    }
}
