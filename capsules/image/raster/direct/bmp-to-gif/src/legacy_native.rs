//! Dependency-free GIF87a/GIF89a and Windows ICO/CUR primitives.
//!
//! GIF decoding covers global/local palettes, LZW, interlace, transparency,
//! frame timing and disposal methods 0..=3. ICO/CUR parsing validates the
//! complete directory and supports PNG members plus common uncompressed DIB
//! members (1/4/8/24/32 bpp). Encoding deliberately emits a small canonical
//! subset: exact-palette GIF and single-member PNG-backed ICO/CUR.

use std::collections::BTreeMap;
use std::fmt;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<Pixel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GifFrame {
    pub(crate) image: Image,
    pub(crate) delay_centiseconds: u16,
    pub(crate) disposal: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GifFile {
    pub(crate) version: &'static str,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) loop_count: Option<u16>,
    pub(crate) frames: Vec<GifFrame>,
    pub(crate) comments: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind {
    Icon,
    Cursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IconMember<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) planes_or_hotspot_x: u16,
    pub(crate) bit_count_or_hotspot_y: u16,
    pub(crate) payload: &'a [u8],
    pub(crate) png: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    Signature(&'static str),
    Truncated(&'static str),
    Invalid(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
    Overflow(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature(value) => write!(f, "invalid {value} signature"),
            Self::Truncated(value) => write!(f, "truncated {value}"),
            Self::Invalid(value) => write!(f, "invalid {value}"),
            Self::Unsupported(value) => write!(f, "unsupported {value}"),
            Self::Limit(value) => write!(f, "resource limit exceeded: {value}"),
            Self::Overflow(value) => write!(f, "integer overflow while computing {value}"),
        }
    }
}

impl std::error::Error for Error {}

fn need<'a>(bytes: &'a [u8], start: usize, length: usize, what: &'static str) -> Result<&'a [u8], Error> {
    let end = start.checked_add(length).ok_or(Error::Overflow(what))?;
    bytes.get(start..end).ok_or(Error::Truncated(what))
}

fn le16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn le_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn checked_pixels(width: u32, height: u32, max_pixels: u64) -> Result<usize, Error> {
    if width == 0 || height == 0 {
        return Err(Error::Invalid("zero image dimension"));
    }
    let count = (width as u64)
        .checked_mul(height as u64)
        .ok_or(Error::Overflow("pixel count"))?;
    if count > max_pixels {
        return Err(Error::Limit("pixel count"));
    }
    usize::try_from(count).map_err(|_| Error::Overflow("pixel allocation"))
}

fn read_palette(bytes: &[u8], offset: &mut usize, entries: usize) -> Result<Vec<Pixel>, Error> {
    let raw = need(bytes, *offset, entries.checked_mul(3).ok_or(Error::Overflow("GIF palette"))?, "GIF palette")?;
    *offset += raw.len();
    Ok(raw
        .chunks_exact(3)
        .map(|rgb| Pixel { r: rgb[0], g: rgb[1], b: rgb[2], a: 255 })
        .collect())
}

fn read_sub_blocks(bytes: &[u8], offset: &mut usize, max_bytes: usize) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();
    loop {
        let length = *need(bytes, *offset, 1, "GIF sub-block length")?
            .first()
            .ok_or(Error::Truncated("GIF sub-block length"))? as usize;
        *offset += 1;
        if length == 0 {
            break;
        }
        let block = need(bytes, *offset, length, "GIF sub-block payload")?;
        *offset += length;
        if result.len().checked_add(length).is_none_or(|value| value > max_bytes) {
            return Err(Error::Limit("GIF sub-block bytes"));
        }
        result.extend_from_slice(block);
    }
    Ok(result)
}

struct GifBits<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl<'a> GifBits<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }

    fn read(&mut self, width: u8) -> Result<u16, Error> {
        let end = self.bit.checked_add(width as usize).ok_or(Error::Overflow("GIF LZW bit offset"))?;
        if end > self.bytes.len() * 8 {
            return Err(Error::Truncated("GIF LZW code"));
        }
        let mut value = 0u16;
        for shift in 0..width as usize {
            let absolute = self.bit + shift;
            value |= (((self.bytes[absolute / 8] >> (absolute % 8)) & 1) as u16) << shift;
        }
        self.bit = end;
        Ok(value)
    }
}

fn reset_dictionary(clear: usize) -> Vec<Vec<u8>> {
    let mut dictionary = Vec::with_capacity(4096);
    for value in 0..clear {
        dictionary.push(vec![value as u8]);
    }
    dictionary.push(Vec::new());
    dictionary.push(Vec::new());
    dictionary
}

fn decode_lzw(data: &[u8], minimum_code_size: u8, expected: usize) -> Result<Vec<u8>, Error> {
    if !(2..=8).contains(&minimum_code_size) {
        return Err(Error::Invalid("GIF LZW minimum code size"));
    }
    let clear = 1usize << minimum_code_size;
    let end = clear + 1;
    let mut dictionary = reset_dictionary(clear);
    let mut width = minimum_code_size + 1;
    let mut next = end + 1;
    let mut previous: Option<Vec<u8>> = None;
    let mut output = Vec::with_capacity(expected);
    let mut bits = GifBits::new(data);
    let mut ended = false;
    while output.len() <= expected {
        let code = bits.read(width)? as usize;
        if code == clear {
            dictionary = reset_dictionary(clear);
            width = minimum_code_size + 1;
            next = end + 1;
            previous = None;
            continue;
        }
        if code == end {
            ended = true;
            break;
        }
        let entry = if code < dictionary.len() && !dictionary[code].is_empty() {
            dictionary[code].clone()
        } else if code == next {
            let mut value = previous.clone().ok_or(Error::Invalid("GIF LZW first dictionary reference"))?;
            let first = *value.first().ok_or(Error::Invalid("GIF LZW empty previous code"))?;
            value.push(first);
            value
        } else {
            return Err(Error::Invalid("GIF LZW dictionary reference"));
        };
        if output.len().checked_add(entry.len()).is_none_or(|value| value > expected) {
            return Err(Error::Invalid("GIF LZW output length"));
        }
        output.extend_from_slice(&entry);
        if let Some(old) = previous {
            if next < 4096 {
                let mut value = old;
                value.push(*entry.first().ok_or(Error::Invalid("GIF LZW empty entry"))?);
                if dictionary.len() == next {
                    dictionary.push(value);
                } else if dictionary.len() > next {
                    dictionary[next] = value;
                } else {
                    return Err(Error::Invalid("GIF LZW dictionary gap"));
                }
                next += 1;
                if next == (1usize << width) && width < 12 {
                    width += 1;
                }
            }
        }
        previous = Some(entry);
    }
    if !ended || output.len() != expected {
        return Err(Error::Invalid("GIF LZW termination or output length"));
    }
    Ok(output)
}

fn interlace_rows(height: usize) -> Vec<usize> {
    let mut rows = Vec::with_capacity(height);
    for (start, step) in [(0usize, 8usize), (4, 8), (2, 4), (1, 2)] {
        let mut row = start;
        while row < height {
            rows.push(row);
            row += step;
        }
    }
    rows
}

#[derive(Debug, Clone, Copy, Default)]
struct GraphicControl {
    disposal: u8,
    delay: u16,
    transparent: Option<u8>,
}

#[derive(Debug, Clone)]
struct PreviousFrame {
    disposal: u8,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    saved: Option<Vec<Pixel>>,
    disposal_background: Pixel,
}

fn fill_rect(canvas: &mut [Pixel], canvas_width: usize, previous: &PreviousFrame, value: Pixel) {
    for y in previous.top..previous.top + previous.height {
        let start = y * canvas_width + previous.left;
        canvas[start..start + previous.width].fill(value);
    }
}

pub(crate) fn decode_gif(
    bytes: &[u8],
    max_pixels: u64,
    max_frames: u32,
    max_sub_block_bytes: usize,
) -> Result<GifFile, Error> {
    let version = match need(bytes, 0, 6, "GIF header")? {
        b"GIF87a" => "GIF87a",
        b"GIF89a" => "GIF89a",
        _ => return Err(Error::Signature("GIF")),
    };
    let screen = need(bytes, 6, 7, "GIF logical screen descriptor")?;
    let width = le16(&screen[0..2]) as u32;
    let height = le16(&screen[2..4]) as u32;
    let count = checked_pixels(width, height, max_pixels)?;
    let packed = screen[4];
    let background_index = screen[5] as usize;
    let mut offset = 13usize;
    let global = if packed & 0x80 != 0 {
        Some(read_palette(bytes, &mut offset, 1usize << ((packed & 7) + 1))?)
    } else {
        None
    };
    let background = global
        .as_ref()
        .and_then(|palette| palette.get(background_index))
        .copied()
        .unwrap_or(Pixel { r: 0, g: 0, b: 0, a: 0 });
    let mut canvas = vec![background; count];
    let mut control = GraphicControl::default();
    let mut previous: Option<PreviousFrame> = None;
    let mut frames = Vec::new();
    let mut loop_count = None;
    let mut comments = 0usize;
    let mut trailer = false;
    while offset < bytes.len() {
        let marker = bytes[offset];
        offset += 1;
        match marker {
            0x3b => {
                trailer = true;
                break;
            }
            0x21 => {
                let label = *need(bytes, offset, 1, "GIF extension label")?
                    .first()
                    .ok_or(Error::Truncated("GIF extension label"))?;
                offset += 1;
                match label {
                    0xf9 => {
                        let size = *need(bytes, offset, 1, "GIF graphic-control size")?
                            .first()
                            .ok_or(Error::Truncated("GIF graphic-control size"))? as usize;
                        offset += 1;
                        if size != 4 {
                            return Err(Error::Invalid("GIF graphic-control size"));
                        }
                        let raw = need(bytes, offset, 4, "GIF graphic-control payload")?;
                        offset += 4;
                        if *need(bytes, offset, 1, "GIF graphic-control terminator")?
                            .first()
                            .ok_or(Error::Truncated("GIF graphic-control terminator"))?
                            != 0
                        {
                            return Err(Error::Invalid("GIF graphic-control terminator"));
                        }
                        offset += 1;
                        let disposal = (raw[0] >> 2) & 7;
                        if disposal > 3 {
                            return Err(Error::Unsupported("GIF disposal method above 3"));
                        }
                        control = GraphicControl {
                            disposal,
                            delay: le16(&raw[1..3]),
                            transparent: (raw[0] & 1 != 0).then_some(raw[3]),
                        };
                    }
                    0xff => {
                        let size = *need(bytes, offset, 1, "GIF application size")?
                            .first()
                            .ok_or(Error::Truncated("GIF application size"))? as usize;
                        offset += 1;
                        let identifier = need(bytes, offset, size, "GIF application identifier")?;
                        offset += size;
                        let data = read_sub_blocks(bytes, &mut offset, max_sub_block_bytes)?;
                        if (identifier == b"NETSCAPE2.0" || identifier == b"ANIMEXTS1.0")
                            && data.len() >= 3
                            && data[0] == 1
                        {
                            loop_count = Some(le16(&data[1..3]));
                        }
                    }
                    0xfe => {
                        read_sub_blocks(bytes, &mut offset, max_sub_block_bytes)?;
                        comments += 1;
                    }
                    0x01 => {
                        return Err(Error::Unsupported("GIF plain-text rendering extension"));
                    }
                    _ => {
                        read_sub_blocks(bytes, &mut offset, max_sub_block_bytes)?;
                    }
                }
            }
            0x2c => {
                if frames.len() >= max_frames as usize {
                    return Err(Error::Limit("GIF frame count"));
                }
                if let Some(state) = previous.take() {
                    match state.disposal {
                        2 => fill_rect(
                            &mut canvas,
                            width as usize,
                            &state,
                            state.disposal_background,
                        ),
                        3 => {
                            if let Some(saved) = state.saved {
                                canvas = saved;
                            }
                        }
                        _ => {}
                    }
                }
                let descriptor = need(bytes, offset, 9, "GIF image descriptor")?;
                offset += 9;
                let left = le16(&descriptor[0..2]) as usize;
                let top = le16(&descriptor[2..4]) as usize;
                let frame_width = le16(&descriptor[4..6]) as usize;
                let frame_height = le16(&descriptor[6..8]) as usize;
                if frame_width == 0
                    || frame_height == 0
                    || left.checked_add(frame_width).is_none_or(|value| value > width as usize)
                    || top.checked_add(frame_height).is_none_or(|value| value > height as usize)
                {
                    return Err(Error::Invalid("GIF frame rectangle"));
                }
                let image_packed = descriptor[8];
                let local = if image_packed & 0x80 != 0 {
                    Some(read_palette(bytes, &mut offset, 1usize << ((image_packed & 7) + 1))?)
                } else {
                    None
                };
                let palette = local.as_ref().or(global.as_ref()).ok_or(Error::Invalid("GIF missing color table"))?;
                let minimum = *need(bytes, offset, 1, "GIF LZW minimum code size")?
                    .first()
                    .ok_or(Error::Truncated("GIF LZW minimum code size"))?;
                offset += 1;
                let compressed = read_sub_blocks(bytes, &mut offset, max_sub_block_bytes)?;
                let frame_count = frame_width
                    .checked_mul(frame_height)
                    .ok_or(Error::Overflow("GIF frame pixels"))?;
                let decoded = decode_lzw(&compressed, minimum, frame_count)?;
                let rows = if image_packed & 0x40 != 0 {
                    interlace_rows(frame_height)
                } else {
                    (0..frame_height).collect()
                };
                if frames.is_empty() && control.transparent.is_some() {
                    canvas.fill(Pixel { r: 0, g: 0, b: 0, a: 0 });
                }
                let saved = (control.disposal == 3).then(|| canvas.clone());
                for (source_row, &target_row) in rows.iter().enumerate() {
                    for x in 0..frame_width {
                        let index = decoded[source_row * frame_width + x];
                        if control.transparent == Some(index) {
                            continue;
                        }
                        let color = *palette.get(index as usize).ok_or(Error::Invalid("GIF palette index"))?;
                        canvas[(top + target_row) * width as usize + left + x] = color;
                    }
                }
                frames.push(GifFrame {
                    image: Image { width, height, pixels: canvas.clone() },
                    delay_centiseconds: control.delay,
                    disposal: control.disposal,
                });
                previous = Some(PreviousFrame {
                    disposal: control.disposal,
                    left,
                    top,
                    width: frame_width,
                    height: frame_height,
                    saved,
                    disposal_background: if control.transparent.is_some() {
                        Pixel { r: 0, g: 0, b: 0, a: 0 }
                    } else {
                        background
                    },
                });
                control = GraphicControl::default();
            }
            _ => return Err(Error::Invalid("GIF block marker")),
        }
    }
    if !trailer || frames.is_empty() {
        return Err(Error::Invalid("GIF trailer or frame set"));
    }
    if offset != bytes.len() {
        return Err(Error::Invalid("GIF trailing data"));
    }
    Ok(GifFile { version, width, height, loop_count, frames, comments })
}

struct GifWriter {
    bytes: Vec<u8>,
    value: u32,
    bits: u8,
}

impl GifWriter {
    fn new() -> Self {
        Self { bytes: Vec::new(), value: 0, bits: 0 }
    }

    fn code(&mut self, code: u16, width: u8) {
        self.value |= (code as u32) << self.bits;
        self.bits += width;
        while self.bits >= 8 {
            self.bytes.push(self.value as u8);
            self.value >>= 8;
            self.bits -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits != 0 {
            self.bytes.push(self.value as u8);
        }
        self.bytes
    }
}

fn write_sub_blocks(out: &mut Vec<u8>, bytes: &[u8]) {
    for block in bytes.chunks(255) {
        out.push(block.len() as u8);
        out.extend_from_slice(block);
    }
    out.push(0);
}

fn encode_literal_lzw(indices: &[u8], minimum_code_size: u8) -> Vec<u8> {
    let clear = 1u16 << minimum_code_size;
    let end = clear + 1;
    let mut width = minimum_code_size + 1;
    let mut next = end as usize + 1;
    let mut previous = false;
    let mut codes = GifWriter::new();
    codes.code(clear, width);
    for &index in indices {
        codes.code(index as u16, width);
        if previous && next < 4096 {
            next += 1;
            if next == (1usize << width) && width < 12 {
                width += 1;
            }
        }
        previous = true;
    }
    codes.code(end, width);
    codes.finish()
}

pub(crate) fn encode_gif(image: &Image) -> Result<Vec<u8>, Error> {
    let count = checked_pixels(image.width, image.height, u64::MAX)?;
    if image.pixels.len() != count {
        return Err(Error::Invalid("GIF encoder pixel count"));
    }
    if image.width > u16::MAX as u32 || image.height > u16::MAX as u32 {
        return Err(Error::Limit("GIF dimensions"));
    }
    let mut palette = Vec::<Pixel>::new();
    let mut lookup = BTreeMap::<Pixel, u8>::new();
    let mut indices = Vec::with_capacity(count);
    let mut transparent = None;
    for &pixel in &image.pixels {
        if pixel.a != 0 && pixel.a != 255 {
            return Err(Error::Unsupported("GIF partial alpha without quantization"));
        }
        if pixel.a == 0 && (pixel.r != 0 || pixel.g != 0 || pixel.b != 0) {
            return Err(Error::Unsupported("GIF transparent nonzero RGB code values"));
        }
        let canonical = if pixel.a == 0 { Pixel { r: 0, g: 0, b: 0, a: 0 } } else { pixel };
        let index = if let Some(&value) = lookup.get(&canonical) {
            value
        } else {
            if palette.len() == 256 {
                return Err(Error::Unsupported("GIF exact palette requires at most 256 colors"));
            }
            let value = palette.len() as u8;
            if canonical.a == 0 {
                transparent = Some(value);
            }
            lookup.insert(canonical, value);
            palette.push(canonical);
            value
        };
        indices.push(index);
    }
    while palette.len() < 2 {
        palette.push(Pixel { r: 0, g: 0, b: 0, a: 255 });
    }
    let table_entries = palette.len().next_power_of_two().min(256);
    let table_bits = table_entries.trailing_zeros() as u8;
    let minimum_code_size = table_bits.max(2);
    let compressed = encode_literal_lzw(&indices, minimum_code_size);
    let mut out = b"GIF89a".to_vec();
    out.extend_from_slice(&(image.width as u16).to_le_bytes());
    out.extend_from_slice(&(image.height as u16).to_le_bytes());
    out.push(0x80 | (7 << 4) | (table_bits - 1));
    out.extend_from_slice(&[0, 0]);
    for index in 0..table_entries {
        let pixel = palette.get(index).copied().unwrap_or_default();
        out.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
    }
    if let Some(index) = transparent {
        out.extend_from_slice(&[0x21, 0xf9, 4, 1, 0, 0, index, 0]);
    }
    out.push(0x2c);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(image.width as u16).to_le_bytes());
    out.extend_from_slice(&(image.height as u16).to_le_bytes());
    out.push(0);
    out.push(minimum_code_size);
    write_sub_blocks(&mut out, &compressed);
    out.push(0x3b);
    Ok(out)
}

pub(crate) fn parse_icon<'a>(
    bytes: &'a [u8],
    expected: IconKind,
    max_members: u32,
) -> Result<Vec<IconMember<'a>>, Error> {
    let header = need(bytes, 0, 6, "ICO/CUR header")?;
    if le16(&header[0..2]) != 0 {
        return Err(Error::Signature("ICO/CUR"));
    }
    let kind = match le16(&header[2..4]) {
        1 => IconKind::Icon,
        2 => IconKind::Cursor,
        _ => return Err(Error::Signature("ICO/CUR")),
    };
    if kind != expected {
        return Err(Error::Signature(match expected { IconKind::Icon => "ICO", IconKind::Cursor => "CUR" }));
    }
    let count = le16(&header[4..6]) as usize;
    if count == 0 || count > max_members as usize {
        return Err(Error::Limit("ICO/CUR member count"));
    }
    let directory_end = 6usize
        .checked_add(count.checked_mul(16).ok_or(Error::Overflow("ICO/CUR directory"))?)
        .ok_or(Error::Overflow("ICO/CUR directory"))?;
    need(bytes, 0, directory_end, "ICO/CUR directory")?;
    let mut members = Vec::with_capacity(count);
    let mut ranges = Vec::with_capacity(count);
    for index in 0..count {
        let entry = &bytes[6 + index * 16..6 + (index + 1) * 16];
        let width = if entry[0] == 0 { 256 } else { entry[0] as u32 };
        let height = if entry[1] == 0 { 256 } else { entry[1] as u32 };
        let size = le32(&entry[8..12]) as usize;
        let offset = le32(&entry[12..16]) as usize;
        if size == 0 || offset < directory_end {
            return Err(Error::Invalid("ICO/CUR member range"));
        }
        let payload = need(bytes, offset, size, "ICO/CUR member payload")?;
        let end = offset.checked_add(size).ok_or(Error::Overflow("ICO/CUR member range"))?;
        for &(other_start, other_end) in &ranges {
            if offset < other_end && other_start < end {
                return Err(Error::Invalid("overlapping ICO/CUR member ranges"));
            }
        }
        ranges.push((offset, end));
        members.push(IconMember {
            width,
            height,
            planes_or_hotspot_x: le16(&entry[4..6]),
            bit_count_or_hotspot_y: le16(&entry[6..8]),
            payload,
            png: payload.starts_with(PNG_SIGNATURE),
        });
    }
    Ok(members)
}

pub(crate) fn select_best<'a>(members: &'a [IconMember<'a>]) -> Result<&'a IconMember<'a>, Error> {
    members
        .iter()
        .max_by_key(|member| {
            let area = member.width as u64 * member.height as u64;
            let depth = if member.png { 65_535 } else { member.bit_count_or_hotspot_y as u64 };
            (area, depth)
        })
        .ok_or(Error::Invalid("empty ICO/CUR member set"))
}

fn dib_palette(payload: &[u8], header_size: usize, entries: usize) -> Result<Vec<Pixel>, Error> {
    let bytes = need(
        payload,
        header_size,
        entries.checked_mul(4).ok_or(Error::Overflow("ICO DIB palette"))?,
        "ICO DIB palette",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|entry| Pixel { r: entry[2], g: entry[1], b: entry[0], a: 255 })
        .collect())
}

fn packed_index(row: &[u8], x: usize, bit_count: u16) -> u8 {
    match bit_count {
        1 => (row[x / 8] >> (7 - x % 8)) & 1,
        4 => {
            if x % 2 == 0 { row[x / 2] >> 4 } else { row[x / 2] & 15 }
        }
        8 => row[x],
        _ => 0,
    }
}

pub(crate) fn decode_dib(payload: &[u8], max_pixels: u64) -> Result<Image, Error> {
    let header_size = le32(need(payload, 0, 4, "ICO DIB header size")?) as usize;
    if header_size < 40 {
        return Err(Error::Unsupported("ICO DIB header smaller than BITMAPINFOHEADER"));
    }
    let header = need(payload, 0, header_size, "ICO DIB header")?;
    let width_raw = le_i32(&header[4..8]);
    let stored_height = le_i32(&header[8..12]);
    if width_raw <= 0 || stored_height == 0 || stored_height == i32::MIN {
        return Err(Error::Invalid("ICO DIB dimensions"));
    }
    let top_down = stored_height < 0;
    let absolute_height = stored_height.unsigned_abs();
    if absolute_height % 2 != 0 {
        return Err(Error::Invalid("ICO DIB doubled height"));
    }
    let width = width_raw as u32;
    let height = absolute_height / 2;
    let count = checked_pixels(width, height, max_pixels)?;
    if le16(&header[12..14]) != 1 {
        return Err(Error::Invalid("ICO DIB planes"));
    }
    let bit_count = le16(&header[14..16]);
    if !matches!(bit_count, 1 | 4 | 8 | 24 | 32) {
        return Err(Error::Unsupported("ICO DIB bit depth"));
    }
    if le32(&header[16..20]) != 0 {
        return Err(Error::Unsupported("compressed ICO DIB"));
    }
    let palette_entries = if bit_count <= 8 {
        let declared = le32(&header[32..36]) as usize;
        if declared == 0 { 1usize << bit_count } else { declared }
    } else {
        0
    };
    if palette_entries > 256 {
        return Err(Error::Invalid("ICO DIB palette length"));
    }
    let palette = dib_palette(payload, header_size, palette_entries)?;
    let xor_offset = header_size
        .checked_add(palette_entries.checked_mul(4).ok_or(Error::Overflow("ICO DIB palette"))?)
        .ok_or(Error::Overflow("ICO DIB XOR offset"))?;
    let xor_row = ((width as usize * bit_count as usize + 31) / 32) * 4;
    let xor_size = xor_row.checked_mul(height as usize).ok_or(Error::Overflow("ICO DIB XOR bitmap"))?;
    let xor = need(payload, xor_offset, xor_size, "ICO DIB XOR bitmap")?;
    let mask_row = ((width as usize + 31) / 32) * 4;
    let mask_size = mask_row.checked_mul(height as usize).ok_or(Error::Overflow("ICO DIB AND mask"))?;
    let mask = need(payload, xor_offset + xor_size, mask_size, "ICO DIB AND mask")?;
    let mut pixels = vec![Pixel::default(); count];
    let any_alpha = bit_count == 32 && xor.chunks_exact(4).any(|bgra| bgra[3] != 0);
    for y in 0..height as usize {
        let source_y = if top_down { y } else { height as usize - 1 - y };
        let xor_row_bytes = &xor[source_y * xor_row..(source_y + 1) * xor_row];
        let mask_row_bytes = &mask[source_y * mask_row..(source_y + 1) * mask_row];
        for x in 0..width as usize {
            let mut pixel = match bit_count {
                1 | 4 | 8 => *palette
                    .get(packed_index(xor_row_bytes, x, bit_count) as usize)
                    .ok_or(Error::Invalid("ICO DIB palette index"))?,
                24 => {
                    let base = x * 3;
                    Pixel { r: xor_row_bytes[base + 2], g: xor_row_bytes[base + 1], b: xor_row_bytes[base], a: 255 }
                }
                32 => {
                    let base = x * 4;
                    Pixel {
                        r: xor_row_bytes[base + 2],
                        g: xor_row_bytes[base + 1],
                        b: xor_row_bytes[base],
                        a: xor_row_bytes[base + 3],
                    }
                }
                _ => unreachable!(),
            };
            let masked = ((mask_row_bytes[x / 8] >> (7 - x % 8)) & 1) != 0;
            if bit_count != 32 || !any_alpha {
                pixel.a = if masked { 0 } else { 255 };
            } else if masked {
                pixel.a = 0;
            }
            pixels[y * width as usize + x] = pixel;
        }
    }
    Ok(Image { width, height, pixels })
}

pub(crate) fn encode_icon(
    png: &[u8],
    width: u32,
    height: u32,
    kind: IconKind,
    hotspot_x: u16,
    hotspot_y: u16,
) -> Result<Vec<u8>, Error> {
    if png.len() > u32::MAX as usize || width == 0 || height == 0 || width > 256 || height > 256 {
        return Err(Error::Limit("ICO/CUR PNG member dimensions or size"));
    }
    if kind == IconKind::Cursor && (hotspot_x as u32 >= width || hotspot_y as u32 >= height) {
        return Err(Error::Invalid("CUR hotspot outside image"));
    }
    let mut out = Vec::with_capacity(22 + png.len());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(if kind == IconKind::Icon { 1u16 } else { 2u16 }).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.push(if width == 256 { 0 } else { width as u8 });
    out.push(if height == 256 { 0 } else { height as u8 });
    out.extend_from_slice(&[0, 0]);
    match kind {
        IconKind::Icon => {
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&32u16.to_le_bytes());
        }
        IconKind::Cursor => {
            out.extend_from_slice(&hotspot_x.to_le_bytes());
            out.extend_from_slice(&hotspot_y.to_le_bytes());
        }
    }
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes());
    out.extend_from_slice(png);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_image() -> Image {
        Image {
            width: 3,
            height: 2,
            pixels: vec![
                Pixel { r: 0, g: 0, b: 0, a: 0 },
                Pixel { r: 255, g: 0, b: 0, a: 255 },
                Pixel { r: 0, g: 255, b: 0, a: 255 },
                Pixel { r: 0, g: 0, b: 255, a: 255 },
                Pixel { r: 255, g: 255, b: 0, a: 255 },
                Pixel { r: 255, g: 255, b: 255, a: 255 },
            ],
        }
    }

    #[test]
    fn gif_exact_palette_and_binary_alpha_round_trip() {
        let image = sample_image();
        let encoded = encode_gif(&image).unwrap();
        let decoded = decode_gif(&encoded, 100, 10, 10_000).unwrap();
        assert_eq!(decoded.frames.len(), 1);
        assert_eq!(decoded.frames[0].image, image);
    }

    #[test]
    fn gif_lzw_width_growth_round_trips_past_twelve_bits() {
        let mut pixels = Vec::new();
        for index in 0..8192u32 {
            let value = index as u8;
            pixels.push(Pixel {
                r: value,
                g: value.wrapping_mul(37),
                b: value.wrapping_mul(91),
                a: 255,
            });
        }
        let image = Image { width: 4096, height: 2, pixels };
        let encoded = encode_gif(&image).unwrap();
        let decoded = decode_gif(&encoded, 10_000, 10, 100_000).unwrap();
        assert_eq!(decoded.frames[0].image, image);
    }

    #[test]
    fn gif_animation_is_not_collapsed_by_the_decoder() {
        let image = sample_image();
        let mut encoded = encode_gif(&image).unwrap();
        let table_entries = 1usize << ((encoded[10] & 7) + 1);
        let frame_start = 13 + table_entries * 3 + 8;
        let frame = encoded[frame_start..encoded.len() - 1].to_vec();
        encoded.pop();
        encoded.extend_from_slice(&frame);
        encoded.push(0x3b);
        let decoded = decode_gif(&encoded, 100, 10, 10_000).unwrap();
        assert_eq!(decoded.frames.len(), 2);
    }

    #[test]
    fn interlace_pass_order_visits_each_row_once() {
        assert_eq!(interlace_rows(5), vec![0, 4, 2, 1, 3]);
        assert_eq!(interlace_rows(8), vec![0, 4, 2, 6, 1, 3, 5, 7]);
    }

    #[test]
    fn ico_dib_32_bit_alpha_and_mask_are_applied() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&8u32.to_le_bytes());
        dib.extend_from_slice(&[0; 16]);
        dib.extend_from_slice(&[30, 20, 10, 128, 60, 50, 40, 255]);
        dib.extend_from_slice(&[0x40, 0, 0, 0]);
        let image = decode_dib(&dib, 10).unwrap();
        assert_eq!(image.pixels[0], Pixel { r: 10, g: 20, b: 30, a: 128 });
        assert_eq!(image.pixels[1], Pixel { r: 40, g: 50, b: 60, a: 0 });
    }

    #[test]
    fn ico_overlapping_member_ranges_are_rejected() {
        let mut ico = vec![0, 0, 1, 0, 2, 0];
        for _ in 0..2 {
            ico.extend_from_slice(&[1, 1, 0, 0, 1, 0, 32, 0]);
            ico.extend_from_slice(&4u32.to_le_bytes());
            ico.extend_from_slice(&38u32.to_le_bytes());
        }
        ico.extend_from_slice(&[1, 2, 3, 4]);
        assert!(matches!(
            parse_icon(&ico, IconKind::Icon, 10),
            Err(Error::Invalid("overlapping ICO/CUR member ranges"))
        ));
    }
}
