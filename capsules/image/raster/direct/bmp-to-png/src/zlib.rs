use std::io::{self, Write};

const MOD_ADLER: u32 = 65_521;
const STORED_BLOCK_MAX: usize = u16::MAX as usize;

#[derive(Debug, Clone, Copy)]
struct Adler32 {
    a: u32,
    b: u32,
}

impl Adler32 {
    fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    fn update(&mut self, bytes: &[u8]) {
        // Periodic reduction keeps sums bounded even for very large rows.
        for chunk in bytes.chunks(5_552) {
            for byte in chunk {
                self.a += *byte as u32;
                self.b += self.a;
            }
            self.a %= MOD_ADLER;
            self.b %= MOD_ADLER;
        }
    }

    fn finish(self) -> u32 {
        (self.b << 16) | self.a
    }
}

pub(crate) struct StoredDeflater<'a, W: Write + ?Sized> {
    output: &'a mut W,
    pending: Vec<u8>,
    adler: Adler32,
}

impl<'a, W: Write + ?Sized> StoredDeflater<'a, W> {
    pub(crate) fn new(output: &'a mut W) -> io::Result<Self> {
        // CM=Deflate, CINFO=32 KiB, fastest compression level, FCHECK valid.
        output.write_all(&[0x78, 0x01])?;
        Ok(Self {
            output,
            pending: Vec::with_capacity(STORED_BLOCK_MAX),
            adler: Adler32::new(),
        })
    }

    pub(crate) fn write_bytes(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        self.adler.update(bytes);
        while !bytes.is_empty() {
            if self.pending.len() == STORED_BLOCK_MAX {
                self.flush_block(false)?;
            }
            let available = STORED_BLOCK_MAX - self.pending.len();
            let take = available.min(bytes.len());
            self.pending.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
        }
        Ok(())
    }

    fn flush_block(&mut self, final_block: bool) -> io::Result<()> {
        self.output.write_all(&[u8::from(final_block)])?;
        let length = self.pending.len() as u16;
        self.output.write_all(&length.to_le_bytes())?;
        self.output.write_all(&(!length).to_le_bytes())?;
        self.output.write_all(&self.pending)?;
        self.pending.clear();
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.flush_block(true)?;
        self.output.write_all(&self.adler.finish().to_be_bytes())
    }
}

struct BitWriter<'a, W: Write + ?Sized> {
    output: &'a mut W,
    bits: u64,
    bit_count: u8,
}

impl<'a, W: Write + ?Sized> BitWriter<'a, W> {
    fn new(output: &'a mut W) -> Self {
        Self {
            output,
            bits: 0,
            bit_count: 0,
        }
    }

    fn write_bits(&mut self, value: u32, count: u8) -> io::Result<()> {
        self.bits |= (value as u64) << self.bit_count;
        self.bit_count += count;
        while self.bit_count >= 8 {
            self.output.write_all(&[self.bits as u8])?;
            self.bits >>= 8;
            self.bit_count -= 8;
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<&'a mut W> {
        if self.bit_count != 0 {
            self.output.write_all(&[self.bits as u8])?;
            self.bits = 0;
            self.bit_count = 0;
        }
        Ok(self.output)
    }
}

fn reverse_code(code: u32, bits: u8) -> u32 {
    code.reverse_bits() >> (32 - bits)
}

fn fixed_symbol<W: Write + ?Sized>(
    output: &mut BitWriter<'_, W>,
    symbol: u16,
) -> io::Result<()> {
    let (code, bits) = match symbol {
        0..=143 => (0x30 + symbol as u32, 8),
        144..=255 => (0x190 + (symbol as u32 - 144), 9),
        256..=279 => (symbol as u32 - 256, 7),
        280..=287 => (0xC0 + (symbol as u32 - 280), 8),
        _ => unreachable!("fixed Huffman literal/length symbol out of range"),
    };
    output.write_bits(reverse_code(code, bits), bits)
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51,
    59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4,
    4, 4, 5, 5, 5, 5, 0,
];

fn fixed_distance_one_match<W: Write + ?Sized>(
    output: &mut BitWriter<'_, W>,
    length: usize,
) -> io::Result<()> {
    debug_assert!((3..=258).contains(&length));
    let mut selected = None;
    for index in 0..LENGTH_BASE.len() {
        let upper = if index == LENGTH_BASE.len() - 1 {
            LENGTH_BASE[index]
        } else {
            LENGTH_BASE[index + 1] - 1
        };
        if length >= LENGTH_BASE[index] as usize && length <= upper as usize {
            selected = Some(index);
            break;
        }
    }
    let index = selected.expect("valid Deflate match length has a code");
    fixed_symbol(output, 257 + index as u16)?;
    let extra_bits = LENGTH_EXTRA[index];
    if extra_bits != 0 {
        output.write_bits(length as u32 - LENGTH_BASE[index] as u32, extra_bits)?;
    }
    // Fixed distance symbol zero means distance one. Its reversed five-bit
    // Huffman code and its extra-bit count are both zero.
    output.write_bits(0, 5)
}

pub(crate) struct FixedRleDeflater<'a, W: Write + ?Sized> {
    bits: BitWriter<'a, W>,
    adler: Adler32,
    pending_byte: Option<u8>,
    pending_count: usize,
}

impl<'a, W: Write + ?Sized> FixedRleDeflater<'a, W> {
    pub(crate) fn new(output: &'a mut W) -> io::Result<Self> {
        output.write_all(&[0x78, 0x01])?;
        let mut bits = BitWriter::new(output);
        bits.write_bits(1, 1)?; // BFINAL
        bits.write_bits(1, 2)?; // BTYPE=01, fixed Huffman
        Ok(Self {
            bits,
            adler: Adler32::new(),
            pending_byte: None,
            pending_count: 0,
        })
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.adler.update(bytes);
        for byte in bytes {
            match self.pending_byte {
                Some(pending) if pending == *byte => {
                    self.pending_count += 1;
                    // One literal plus a maximum-length match.
                    if self.pending_count == 259 {
                        self.flush_run()?;
                    }
                }
                Some(_) => {
                    self.flush_run()?;
                    self.pending_byte = Some(*byte);
                    self.pending_count = 1;
                }
                None => {
                    self.pending_byte = Some(*byte);
                    self.pending_count = 1;
                }
            }
        }
        Ok(())
    }

    fn flush_run(&mut self) -> io::Result<()> {
        let Some(value) = self.pending_byte.take() else {
            return Ok(());
        };
        fixed_symbol(&mut self.bits, value as u16)?;
        let mut remaining = self.pending_count - 1;
        while remaining >= 3 {
            let length = remaining.min(258);
            fixed_distance_one_match(&mut self.bits, length)?;
            remaining -= length;
        }
        for _ in 0..remaining {
            fixed_symbol(&mut self.bits, value as u16)?;
        }
        self.pending_count = 0;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.flush_run()?;
        fixed_symbol(&mut self.bits, 256)?;
        let adler = self.adler.finish();
        let output = self.bits.finish()?;
        output.write_all(&adler.to_be_bytes())
    }
}
