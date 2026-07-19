//! Dependency-free PNG codec materialized into every PNG Wave B Capsule.
//!
//! The decoder covers all PNG color types and legal bit-depth combinations,
//! stored/fixed/dynamic Deflate blocks, filters 0..=4 and Adam7. The encoder
//! deliberately emits a small canonical subset (RGB/RGBA, 8/16-bit,
//! non-interlaced) so normalization has one deterministic target.

use std::fmt;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
    (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Pixel16 { pub(crate) r: u16, pub(crate) g: u16, pub(crate) b: u16, pub(crate) a: u16 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) source_channels: u8,
    pub(crate) source_bit_depth: u8,
    pub(crate) source_color_type: u8,
    pub(crate) interlaced: bool,
    pub(crate) pixels: Vec<Pixel16>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filter { None, Sub, Up, Average, Paeth, Adaptive }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodeOptions {
    pub(crate) max_pixels: u64,
    pub(crate) max_inflate_bytes: u64,
    pub(crate) strict_crc: bool,
    pub(crate) strict_trailing_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    Signature,
    Truncated(&'static str),
    Invalid(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
    Overflow(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature => write!(f, "invalid PNG signature"),
            Self::Truncated(v) => write!(f, "truncated PNG {v}"),
            Self::Invalid(v) => write!(f, "invalid PNG {v}"),
            Self::Unsupported(v) => write!(f, "unsupported PNG feature: {v}"),
            Self::Limit(v) => write!(f, "PNG resource limit exceeded: {v}"),
            Self::Overflow(v) => write!(f, "integer overflow while computing PNG {v}"),
        }
    }
}

impl std::error::Error for Error {}

fn need<'a>(bytes: &'a [u8], start: usize, len: usize, what: &'static str) -> Result<&'a [u8], Error> {
    let end = start.checked_add(len).ok_or(Error::Overflow(what))?;
    bytes.get(start..end).ok_or(Error::Truncated(what))
}

fn be32(bytes: &[u8]) -> u32 { u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 }; }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5_552) {
        for &byte in chunk { a += byte as u32; b += a; }
        a %= 65_521; b %= 65_521;
    }
    (b << 16) | a
}

struct Bits<'a> { bytes: &'a [u8], pos: usize, value: u8, left: u8 }

impl<'a> Bits<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, pos: 0, value: 0, left: 0 } }
    fn bit(&mut self) -> Result<u32, Error> {
        if self.left == 0 {
            self.value = *self.bytes.get(self.pos).ok_or(Error::Truncated("Deflate bit stream"))?;
            self.pos += 1; self.left = 8;
        }
        let result = (self.value & 1) as u32;
        self.value >>= 1; self.left -= 1;
        Ok(result)
    }
    fn read(&mut self, count: u8) -> Result<u32, Error> {
        let mut result = 0;
        for shift in 0..count { result |= self.bit()? << shift; }
        Ok(result)
    }
    fn align(&mut self) { self.value = 0; self.left = 0; }
    fn byte(&mut self) -> Result<u8, Error> {
        if self.left != 0 { return Err(Error::Invalid("unaligned Deflate byte read")); }
        let result = *self.bytes.get(self.pos).ok_or(Error::Truncated("Deflate byte stream"))?;
        self.pos += 1; Ok(result)
    }
}

#[derive(Clone)]
struct Huffman { by_length: Vec<Vec<(u16, u16)>>, max: u8 }

fn reverse(mut code: u16, length: u8) -> u16 {
    let mut result = 0;
    for _ in 0..length { result = (result << 1) | (code & 1); code >>= 1; }
    result
}

impl Huffman {
    fn new(lengths: &[u8], allow_single: bool) -> Result<Self, Error> {
        let max = lengths.iter().copied().max().unwrap_or(0);
        if max > 15 || max == 0 { return Err(Error::Invalid("empty or overlong Huffman tree")); }
        let mut counts = [0u16; 16];
        for &length in lengths { if length > 0 { counts[length as usize] += 1; } }
        let mut left = 1i32;
        for count in counts.iter().skip(1) { left = left * 2 - *count as i32; if left < 0 { return Err(Error::Invalid("oversubscribed Huffman tree")); } }
        let symbols = lengths.iter().filter(|&&v| v != 0).count();
        if left > 0 && !(allow_single && symbols == 1) { return Err(Error::Invalid("incomplete Huffman tree")); }
        let mut next = [0u16; 16];
        let mut code = 0u16;
        for bits in 1..=15 { code = (code + counts[bits - 1]) << 1; next[bits] = code; }
        let mut by_length = vec![Vec::new(); max as usize + 1];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                let canonical = next[length as usize]; next[length as usize] += 1;
                by_length[length as usize].push((reverse(canonical, length), symbol as u16));
            }
        }
        Ok(Self { by_length, max })
    }
    fn symbol(&self, bits: &mut Bits<'_>) -> Result<u16, Error> {
        let mut code = 0u16;
        for length in 1..=self.max {
            code |= (bits.bit()? as u16) << (length - 1);
            if let Some((_, symbol)) = self.by_length[length as usize].iter().find(|(candidate, _)| *candidate == code) { return Ok(*symbol); }
        }
        Err(Error::Invalid("Huffman code"))
    }
}

fn fixed_trees() -> Result<(Huffman, Huffman), Error> {
    let mut literal = vec![0u8; 288];
    literal[0..144].fill(8); literal[144..256].fill(9); literal[256..280].fill(7); literal[280..288].fill(8);
    Ok((Huffman::new(&literal, false)?, Huffman::new(&[5u8; 32], false)?))
}

fn dynamic_trees(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), Error> {
    let literal_count = bits.read(5)? as usize + 257;
    let distance_count = bits.read(5)? as usize + 1;
    let code_count = bits.read(4)? as usize + 4;
    let order = [16usize,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
    let mut code_lengths = [0u8; 19];
    for index in 0..code_count { code_lengths[order[index]] = bits.read(3)? as u8; }
    let code_tree = Huffman::new(&code_lengths, true)?;
    let total = literal_count.checked_add(distance_count).ok_or(Error::Overflow("dynamic tree size"))?;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        match code_tree.symbol(bits)? {
            value @ 0..=15 => lengths.push(value as u8),
            16 => {
                let previous = *lengths.last().ok_or(Error::Invalid("repeat with no previous code length"))?;
                let count = bits.read(2)? as usize + 3;
                if lengths.len() + count > total { return Err(Error::Invalid("code-length repeat exceeds tree")); }
                lengths.extend(std::iter::repeat_n(previous, count));
            }
            17 => {
                let count = bits.read(3)? as usize + 3;
                if lengths.len() + count > total { return Err(Error::Invalid("zero repeat exceeds tree")); }
                lengths.extend(std::iter::repeat_n(0, count));
            }
            18 => {
                let count = bits.read(7)? as usize + 11;
                if lengths.len() + count > total { return Err(Error::Invalid("long zero repeat exceeds tree")); }
                lengths.extend(std::iter::repeat_n(0, count));
            }
            _ => return Err(Error::Invalid("code-length symbol")),
        }
    }
    if lengths.get(256).copied().unwrap_or(0) == 0 { return Err(Error::Invalid("Deflate tree lacks end-of-block symbol")); }
    let literal = Huffman::new(&lengths[..literal_count], false)?;
    let distance = Huffman::new(&lengths[literal_count..], true)?;
    Ok((literal, distance))
}

const LENGTH_BASE: [usize; 29] = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
const LENGTH_EXTRA: [u8; 29] = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
const DIST_BASE: [usize; 30] = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
const DIST_EXTRA: [u8; 30] = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];

fn compressed_block(bits: &mut Bits<'_>, output: &mut Vec<u8>, literal: &Huffman, distance: &Huffman, max: usize) -> Result<(), Error> {
    loop {
        match literal.symbol(bits)? {
            value @ 0..=255 => {
                if output.len() >= max { return Err(Error::Limit("inflated bytes")); }
                output.push(value as u8);
            }
            256 => return Ok(()),
            value @ 257..=285 => {
                let index = value as usize - 257;
                let length = LENGTH_BASE[index] + bits.read(LENGTH_EXTRA[index])? as usize;
                let distance_symbol = distance.symbol(bits)? as usize;
                if distance_symbol >= 30 { return Err(Error::Invalid("reserved Deflate distance symbol")); }
                let back = DIST_BASE[distance_symbol] + bits.read(DIST_EXTRA[distance_symbol])? as usize;
                if back == 0 || back > output.len() { return Err(Error::Invalid("Deflate distance exceeds history")); }
                if output.len().checked_add(length).ok_or(Error::Overflow("inflated bytes"))? > max { return Err(Error::Limit("inflated bytes")); }
                for _ in 0..length { let byte = output[output.len() - back]; output.push(byte); }
            }
            _ => return Err(Error::Invalid("reserved Deflate literal/length symbol")),
        }
    }
}

fn inflate_zlib(stream: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if stream.len() < 6 { return Err(Error::Truncated("zlib stream")); }
    let cmf = stream[0]; let flg = stream[1];
    if cmf & 15 != 8 || cmf >> 4 > 7 || ((cmf as u16) << 8 | flg as u16) % 31 != 0 { return Err(Error::Invalid("zlib header")); }
    if flg & 0x20 != 0 { return Err(Error::Unsupported("preset zlib dictionary")); }
    let mut bits = Bits::new(&stream[2..]);
    let mut output = Vec::with_capacity(expected);
    loop {
        let final_block = bits.read(1)? != 0;
        match bits.read(2)? {
            0 => {
                bits.align();
                let len = bits.byte()? as u16 | (bits.byte()? as u16) << 8;
                let nlen = bits.byte()? as u16 | (bits.byte()? as u16) << 8;
                if len != !nlen { return Err(Error::Invalid("stored Deflate block length")); }
                if output.len().checked_add(len as usize).ok_or(Error::Overflow("inflated bytes"))? > expected { return Err(Error::Limit("inflated bytes")); }
                for _ in 0..len { output.push(bits.byte()?); }
            }
            1 => { let (literal, distance) = fixed_trees()?; compressed_block(&mut bits, &mut output, &literal, &distance, expected)?; }
            2 => { let (literal, distance) = dynamic_trees(&mut bits)?; compressed_block(&mut bits, &mut output, &literal, &distance, expected)?; }
            _ => return Err(Error::Invalid("reserved Deflate block type")),
        }
        if final_block { break; }
    }
    bits.align();
    let consumed = 2usize.checked_add(bits.pos).ok_or(Error::Overflow("zlib position"))?;
    let checksum = need(stream, consumed, 4, "zlib Adler-32")?;
    if consumed + 4 != stream.len() { return Err(Error::Invalid("trailing zlib bytes")); }
    if be32(checksum) != adler32(&output) { return Err(Error::Invalid("zlib Adler-32")); }
    if output.len() != expected { return Err(Error::Invalid("inflated byte count")); }
    Ok(output)
}

fn channels(color: u8) -> Result<usize, Error> {
    match color { 0 => Ok(1), 2 => Ok(3), 3 => Ok(1), 4 => Ok(2), 6 => Ok(4), _ => Err(Error::Invalid("color type")) }
}

fn valid_depth(color: u8, depth: u8) -> bool {
    matches!((color, depth), (0,1|2|4|8|16) | (2,8|16) | (3,1|2|4|8) | (4,8|16) | (6,8|16))
}

fn pass_size(size: usize, start: usize, step: usize) -> usize { if size <= start { 0 } else { (size - start + step - 1) / step } }

fn expected_inflate(width: usize, height: usize, bits_per_pixel: usize, interlace: u8) -> Result<usize, Error> {
    let passes: &[(usize,usize,usize,usize)] = if interlace == 0 { &[(0,0,1,1)] } else { &ADAM7 };
    let mut total = 0usize;
    for &(x0,y0,dx,dy) in passes {
        let w = pass_size(width, x0, dx); let h = pass_size(height, y0, dy);
        if w == 0 || h == 0 { continue; }
        let row_bits = w.checked_mul(bits_per_pixel).ok_or(Error::Overflow("scanline bits"))?;
        let row = row_bits.checked_add(7).ok_or(Error::Overflow("scanline bytes"))? / 8;
        total = total.checked_add(h.checked_mul(row + 1).ok_or(Error::Overflow("pass bytes"))?).ok_or(Error::Overflow("image bytes"))?;
    }
    Ok(total)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs(); let pb = (p - b as i32).abs(); let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
}

fn unfilter(kind: u8, row: &mut [u8], prior: &[u8], bpp: usize) -> Result<(), Error> {
    if kind > 4 { return Err(Error::Invalid("scanline filter")); }
    for index in 0..row.len() {
        let left = if index >= bpp { row[index - bpp] } else { 0 };
        let up = prior.get(index).copied().unwrap_or(0);
        let upper_left = if index >= bpp { prior.get(index - bpp).copied().unwrap_or(0) } else { 0 };
        let predictor = match kind { 0 => 0, 1 => left, 2 => up, 3 => ((left as u16 + up as u16) / 2) as u8, 4 => paeth(left, up, upper_left), _ => unreachable!() };
        row[index] = row[index].wrapping_add(predictor);
    }
    Ok(())
}

fn sample(row: &[u8], index: usize, depth: u8) -> Result<u16, Error> {
    Ok(match depth {
        16 => { let pos = index.checked_mul(2).ok_or(Error::Overflow("sample offset"))?; u16::from_be_bytes([*row.get(pos).ok_or(Error::Truncated("sample"))?, *row.get(pos + 1).ok_or(Error::Truncated("sample"))?]) }
        8 => *row.get(index).ok_or(Error::Truncated("sample"))? as u16,
        1 | 2 | 4 => {
            let per_byte = 8 / depth as usize; let byte = *row.get(index / per_byte).ok_or(Error::Truncated("packed sample"))?;
            let shift = 8 - depth as usize * (index % per_byte + 1); ((byte >> shift) & ((1u8 << depth) - 1)) as u16
        }
        _ => return Err(Error::Invalid("sample depth")),
    })
}

fn scale16(value: u16, depth: u8) -> u16 {
    if depth == 16 { value } else { let max = (1u32 << depth) - 1; ((value as u32 * 65_535 + max / 2) / max) as u16 }
}

pub(crate) fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<Image, Error> {
    if bytes.get(..8) != Some(SIGNATURE) { return Err(Error::Signature); }
    let mut pos = 8usize; let mut ihdr = None; let mut palette: Vec<[u8;3]> = Vec::new(); let mut transparency = Vec::new(); let mut idat = Vec::new();
    let mut saw_idat = false; let mut ended_idat = false; let mut saw_iend = false; let mut warnings = Vec::new();
    while pos < bytes.len() {
        let header = need(bytes, pos, 8, "chunk header")?; let length = be32(&header[..4]) as usize; let kind = &header[4..8]; pos += 8;
        if !kind.iter().all(u8::is_ascii_alphabetic) || kind[2].is_ascii_lowercase() { return Err(Error::Invalid("chunk type")); }
        let data = need(bytes, pos, length, "chunk data")?; pos += length;
        let stored_crc = be32(need(bytes, pos, 4, "chunk CRC")?); pos += 4;
        if options.strict_crc { let mut crc_data = Vec::with_capacity(4 + length); crc_data.extend_from_slice(kind); crc_data.extend_from_slice(data); if crc32(&crc_data) != stored_crc { return Err(Error::Invalid("chunk CRC")); } }
        if ihdr.is_none() && kind != b"IHDR" { return Err(Error::Invalid("IHDR is not first")); }
        if saw_idat && kind != b"IDAT" && kind != b"IEND" { ended_idat = true; }
        match kind {
            b"IHDR" => { if ihdr.is_some() || length != 13 { return Err(Error::Invalid("IHDR")); } ihdr = Some(data.to_vec()); }
            b"PLTE" => { if saw_idat || !palette.is_empty() || length == 0 || length % 3 != 0 || length > 768 { return Err(Error::Invalid("PLTE")); } palette = data.chunks_exact(3).map(|v| [v[0],v[1],v[2]]).collect(); }
            b"tRNS" => { if saw_idat || !transparency.is_empty() { return Err(Error::Invalid("tRNS")); } transparency.extend_from_slice(data); }
            b"IDAT" => { if ended_idat { return Err(Error::Invalid("non-consecutive IDAT chunks")); } saw_idat = true; idat.extend_from_slice(data); }
            b"IEND" => { if length != 0 || !saw_idat { return Err(Error::Invalid("IEND")); } saw_iend = true; break; }
            _ if kind[0].is_ascii_uppercase() => return Err(Error::Unsupported("unknown critical chunk")),
            _ => warnings.push(format!("ancillary chunk {} discarded by canonical operations", String::from_utf8_lossy(kind))),
        }
    }
    if !saw_iend { return Err(Error::Truncated("IEND")); }
    if options.strict_trailing_data && pos != bytes.len() { return Err(Error::Invalid("bytes after IEND")); }
    let h = ihdr.ok_or(Error::Invalid("missing IHDR"))?;
    let width = be32(&h[0..4]); let height = be32(&h[4..8]); let depth = h[8]; let color = h[9]; let compression = h[10]; let filter = h[11]; let interlace = h[12];
    if width == 0 || height == 0 { return Err(Error::Invalid("zero dimensions")); }
    if !valid_depth(color, depth) { return Err(Error::Invalid("bit depth/color type combination")); }
    if compression != 0 || filter != 0 || interlace > 1 { return Err(Error::Unsupported("IHDR method")); }
    let count64 = width as u64 * height as u64;
    if count64 > options.max_pixels { return Err(Error::Limit("pixel count")); }
    let count = usize::try_from(count64).map_err(|_| Error::Overflow("pixel allocation"))?;
    if color == 3 {
        if palette.is_empty() || palette.len() > (1usize << depth) { return Err(Error::Invalid("indexed image palette")); }
        if transparency.len() > palette.len() { return Err(Error::Invalid("indexed tRNS length")); }
    } else if !palette.is_empty() && matches!(color, 0|4) { return Err(Error::Invalid("PLTE forbidden for grayscale")); }
    match color {
        0 if !transparency.is_empty() && transparency.len() != 2 => return Err(Error::Invalid("grayscale tRNS length")),
        2 if !transparency.is_empty() && transparency.len() != 6 => return Err(Error::Invalid("truecolor tRNS length")),
        4|6 if !transparency.is_empty() => return Err(Error::Invalid("tRNS forbidden with alpha channel")),
        _ => {}
    }
    let channel_count = channels(color)?; let bits_per_pixel = channel_count * depth as usize;
    let expected = expected_inflate(width as usize, height as usize, bits_per_pixel, interlace)?;
    if expected as u64 > options.max_inflate_bytes { return Err(Error::Limit("inflated scanlines")); }
    let raw = inflate_zlib(&idat, expected)?;
    let passes: &[(usize,usize,usize,usize)] = if interlace == 0 { &[(0,0,1,1)] } else { &ADAM7 };
    let mut pixels = vec![Pixel16::default(); count]; let mut cursor = 0usize;
    let transparent_gray = if color == 0 && transparency.len() == 2 { Some(u16::from_be_bytes([transparency[0], transparency[1]])) } else { None };
    let transparent_rgb = if color == 2 && transparency.len() == 6 { Some((u16::from_be_bytes([transparency[0],transparency[1]]),u16::from_be_bytes([transparency[2],transparency[3]]),u16::from_be_bytes([transparency[4],transparency[5]]))) } else { None };
    for &(x0,y0,dx,dy) in passes {
        let pass_width = pass_size(width as usize, x0, dx); let pass_height = pass_size(height as usize, y0, dy);
        if pass_width == 0 || pass_height == 0 { continue; }
        let row_bytes = (pass_width * bits_per_pixel + 7) / 8; let bpp = ((bits_per_pixel + 7) / 8).max(1); let mut prior = vec![0u8; row_bytes];
        for py in 0..pass_height {
            let kind = *raw.get(cursor).ok_or(Error::Truncated("filter byte"))?; cursor += 1;
            let mut row = need(&raw, cursor, row_bytes, "scanline")?.to_vec(); cursor += row_bytes; unfilter(kind, &mut row, &prior, bpp)?;
            for px in 0..pass_width {
                let base = px * channel_count;
                let pixel = match color {
                    0 => { let g0 = sample(&row, base, depth)?; let g = scale16(g0, depth); Pixel16 { r:g,g,b:g,a:if transparent_gray == Some(g0) {0} else {65_535} } }
                    2 => { let r0=sample(&row,base,depth)?;let g0=sample(&row,base+1,depth)?;let b0=sample(&row,base+2,depth)?;Pixel16{r:scale16(r0,depth),g:scale16(g0,depth),b:scale16(b0,depth),a:if transparent_rgb==Some((r0,g0,b0)){0}else{65_535}} }
                    3 => { let index=sample(&row,base,depth)? as usize;let rgb=*palette.get(index).ok_or(Error::Invalid("palette index"))?;Pixel16{r:rgb[0] as u16*257,g:rgb[1] as u16*257,b:rgb[2] as u16*257,a:transparency.get(index).copied().unwrap_or(255) as u16*257} }
                    4 => { let g=scale16(sample(&row,base,depth)?,depth);Pixel16{r:g,g,b:g,a:scale16(sample(&row,base+1,depth)?,depth)} }
                    6 => Pixel16{r:scale16(sample(&row,base,depth)?,depth),g:scale16(sample(&row,base+1,depth)?,depth),b:scale16(sample(&row,base+2,depth)?,depth),a:scale16(sample(&row,base+3,depth)?,depth)},
                    _ => unreachable!(),
                };
                let x=x0+px*dx;let y=y0+py*dy;pixels[y*width as usize+x]=pixel;
            }
            prior = row;
        }
    }
    if cursor != raw.len() { return Err(Error::Invalid("unused inflated bytes")); }
    Ok(Image { width, height, source_channels: channel_count as u8, source_bit_depth: depth, source_color_type: color, interlaced: interlace == 1, pixels, warnings })
}

fn filter_row(kind: u8, raw: &[u8], prior: &[u8], bpp: usize, out: &mut Vec<u8>) -> u64 {
    let start = out.len(); out.push(kind);
    for index in 0..raw.len() {
        let left=if index>=bpp{raw[index-bpp]}else{0};let up=prior.get(index).copied().unwrap_or(0);let ul=if index>=bpp{prior.get(index-bpp).copied().unwrap_or(0)}else{0};
        let predictor=match kind{0=>0,1=>left,2=>up,3=>((left as u16+up as u16)/2)as u8,4=>paeth(left,up,ul),_=>0};out.push(raw[index].wrapping_sub(predictor));
    }
    out[start+1..].iter().map(|&v| (v as i8 as i16).unsigned_abs() as u64).sum()
}

fn zlib_store(bytes: &[u8]) -> Vec<u8> {
    let mut out=vec![0x78,0x01];
    if bytes.is_empty(){out.extend_from_slice(&[1,0,0,255,255]);}else{let mut pos=0;while pos<bytes.len(){let len=(bytes.len()-pos).min(65_535);out.push(if pos+len==bytes.len(){1}else{0});let n=len as u16;out.extend_from_slice(&n.to_le_bytes());out.extend_from_slice(&(!n).to_le_bytes());out.extend_from_slice(&bytes[pos..pos+len]);pos+=len;}}
    out.extend_from_slice(&adler32(bytes).to_be_bytes());out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8;4], data: &[u8]) -> Result<(), Error> {
    let len=u32::try_from(data.len()).map_err(|_|Error::Overflow("chunk length"))?;out.extend_from_slice(&len.to_be_bytes());out.extend_from_slice(kind);out.extend_from_slice(data);let mut crc_data=Vec::with_capacity(4+data.len());crc_data.extend_from_slice(kind);crc_data.extend_from_slice(data);out.extend_from_slice(&crc32(&crc_data).to_be_bytes());Ok(())
}

pub(crate) fn encode(image: &Image, filter: Filter) -> Result<Vec<u8>, Error> {
    if image.width==0||image.height==0||image.pixels.len()!=image.width as usize*image.height as usize{return Err(Error::Invalid("encoder image dimensions"));}
    let alpha=image.pixels.iter().any(|p|p.a!=65_535);let color=if alpha{6}else{2};let channels=if alpha{4}else{3};let depth=if image.source_bit_depth==16{16}else{8};let bytes_per_sample=if depth==16{2}else{1};let bpp=channels*bytes_per_sample;let row_bytes=image.width as usize*bpp;let mut filtered=Vec::with_capacity(image.height as usize*(row_bytes+1));let mut prior=vec![0u8;row_bytes];
    for row in image.pixels.chunks_exact(image.width as usize){let mut raw=Vec::with_capacity(row_bytes);for pixel in row{for sample in [pixel.r,pixel.g,pixel.b,pixel.a].into_iter().take(channels){if depth==16{raw.extend_from_slice(&sample.to_be_bytes());}else{raw.push(((sample as u32+128)/257)as u8);}}}let choices:&[u8]=match filter{Filter::None=>&[0],Filter::Sub=>&[1],Filter::Up=>&[2],Filter::Average=>&[3],Filter::Paeth=>&[4],Filter::Adaptive=>&[0,1,2,3,4]};let mut best=None;for &kind in choices{let mut candidate=Vec::with_capacity(row_bytes+1);let score=filter_row(kind,&raw,&prior,bpp,&mut candidate);if best.as_ref().is_none_or(|(best_score,_)|score<*best_score){best=Some((score,candidate));}}filtered.extend_from_slice(&best.expect("filter choice").1);prior=raw;}
    let compressed=zlib_store(&filtered);let mut out=Vec::new();out.extend_from_slice(SIGNATURE);let mut ihdr=Vec::with_capacity(13);ihdr.extend_from_slice(&image.width.to_be_bytes());ihdr.extend_from_slice(&image.height.to_be_bytes());ihdr.extend_from_slice(&[depth,color,0,0,0]);chunk(&mut out,b"IHDR",&ihdr)?;for part in compressed.chunks(64*1024){chunk(&mut out,b"IDAT",part)?;}chunk(&mut out,b"IEND",&[])?;Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options()->DecodeOptions{DecodeOptions{max_pixels:1_000,max_inflate_bytes:1_000_000,strict_crc:true,strict_trailing_data:true}}
    fn image(depth:u8)->Image{Image{width:3,height:2,source_channels:4,source_bit_depth:depth,source_color_type:6,interlaced:false,pixels:vec![Pixel16{r:0,g:0,b:0,a:65_535},Pixel16{r:65_535,g:0,b:32768,a:32768},Pixel16{r:257,g:514,b:771,a:65_535},Pixel16{r:1,g:2,b:3,a:4},Pixel16{r:10_000,g:20_000,b:30_000,a:40_000},Pixel16{r:65_535,g:65_535,b:65_535,a:65_535}],warnings:Vec::new()}}
    #[test]fn canonical_roundtrip_8_and_16(){for depth in [8,16]{let original=image(depth);let png=encode(&original,Filter::Adaptive).unwrap();let decoded=decode(&png,&options()).unwrap();if depth==16{assert_eq!(decoded.pixels,original.pixels);}else{assert_eq!(decoded.pixels[2],original.pixels[2]);}assert_eq!((decoded.width,decoded.height),(3,2));}}
    #[test]fn all_encoder_filters_decode(){for filter in [Filter::None,Filter::Sub,Filter::Up,Filter::Average,Filter::Paeth,Filter::Adaptive]{let png=encode(&image(16),filter).unwrap();assert_eq!(decode(&png,&options()).unwrap().pixels,image(16).pixels);}}
    #[test]fn crc_and_adler_are_enforced(){let mut png=encode(&image(8),Filter::None).unwrap();let idat=png.windows(4).position(|v|v==b"IDAT").unwrap();png[idat+8]^=1;assert!(decode(&png,&options()).is_err());}
    #[test]fn pixel_limit_precedes_allocation(){let png=encode(&image(8),Filter::None).unwrap();let mut limited=options();limited.max_pixels=5;assert!(matches!(decode(&png,&limited),Err(Error::Limit("pixel count"))));}
}
