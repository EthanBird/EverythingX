#![forbid(unsafe_code)]

mod png_native;

use std::fmt;
use std::io::{self, Read, Write};

const SOURCE: Profile = Profile::Png;
const TARGET: Profile = Profile::Qoi;
const DEFAULT_MAX_PIXELS: u64 = 100_000_000;
const DEFAULT_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    Bmp,
    Tga,
    Qoi,
    Ppm,
    Pam,
    Png,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::Tga => "tga",
            Self::Qoi => "qoi",
            Self::Ppm => "ppm",
            Self::Pam => "pam",
            Self::Png => "png",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaPolicy {
    /// Reject non-opaque pixels instead of silently losing transparency.
    Reject,
    /// Retain RGB code values and remove the alpha channel.
    Discard,
    /// Composite unassociated RGBA code values over black using integer arithmetic.
    CompositeBlack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub max_pixels: u64,
    pub max_input_bytes: u64,
    pub strict_trailing_data: bool,
    pub ppm_alpha: AlphaPolicy,
    pub preserve_unmarked_bmp_alpha: bool,
    pub allow_sample_scaling: bool,
    pub tga_rle: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_pixels: DEFAULT_MAX_PIXELS,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            strict_trailing_data: true,
            ppm_alpha: AlphaPolicy::Reject,
            preserve_unmarked_bmp_alpha: false,
            allow_sample_scaling: false,
            tga_rle: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: u64,
    pub source_profile: &'static str,
    pub target_profile: &'static str,
    pub source_channels: u8,
    pub target_channels: u8,
    pub non_opaque_pixels: u64,
    pub alpha_action: &'static str,
    pub peak_working_memory_bytes: u64,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    InputTooLarge { bytes: u64, limit: u64 },
    InvalidSignature(&'static str),
    InvalidHeader(&'static str),
    Unsupported(&'static str),
    Truncated(&'static str),
    PixelLimitExceeded { pixels: u64, limit: u64 },
    AlphaNotRepresentable { non_opaque_pixels: u64 },
    TrailingData { bytes: usize },
    IntegerOverflow(&'static str),
    Png(String),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(f, "invalid options: {message}"),
            Self::InputTooLarge { bytes, limit } => write!(f, "input has {bytes} bytes, exceeding {limit}"),
            Self::InvalidSignature(format) => write!(f, "input is not a supported {format} image"),
            Self::InvalidHeader(message) => write!(f, "invalid image header: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported image feature: {message}"),
            Self::Truncated(region) => write!(f, "truncated image {region}"),
            Self::PixelLimitExceeded { pixels, limit } => write!(f, "image has {pixels} pixels, exceeding {limit}"),
            Self::AlphaNotRepresentable { non_opaque_pixels } => write!(f, "target cannot represent alpha for {non_opaque_pixels} pixels under reject policy"),
            Self::TrailingData { bytes } => write!(f, "image has {bytes} unexpected trailing bytes"),
            Self::IntegerOverflow(context) => write!(f, "integer overflow while computing {context}"),
            Self::Png(message) => write!(f, "PNG codec error: {message}"),
            Self::Io(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io(error) => Some(error), _ => None }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self { Self::Io(value) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Pixel { r: u8, g: u8, b: u8, a: u8 }

#[derive(Debug, Clone)]
struct Image {
    width: u32,
    height: u32,
    source_channels: u8,
    pixels: Vec<Pixel>,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct Encoded {
    bytes: Vec<u8>,
    channels: u8,
    alpha_action: &'static str,
}

fn checked_pixels(width: u32, height: u32, options: &Options) -> Result<usize, Error> {
    if width == 0 || height == 0 { return Err(Error::InvalidHeader("width and height must be non-zero")); }
    let count = (width as u64).checked_mul(height as u64).ok_or(Error::IntegerOverflow("pixel count"))?;
    if count > options.max_pixels { return Err(Error::PixelLimitExceeded { pixels: count, limit: options.max_pixels }); }
    usize::try_from(count).map_err(|_| Error::IntegerOverflow("pixel allocation"))
}

fn need<'a>(bytes: &'a [u8], start: usize, length: usize, region: &'static str) -> Result<&'a [u8], Error> {
    let end = start.checked_add(length).ok_or(Error::IntegerOverflow(region))?;
    bytes.get(start..end).ok_or(Error::Truncated(region))
}

fn le_u16(bytes: &[u8]) -> u16 { u16::from_le_bytes([bytes[0], bytes[1]]) }
fn le_u32(bytes: &[u8]) -> u32 { u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }
fn le_i32(bytes: &[u8]) -> i32 { i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }
fn be_u32(bytes: &[u8]) -> u32 { u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }

fn decode(profile: Profile, bytes: &[u8], options: &Options) -> Result<Image, Error> {
    match profile {
        Profile::Bmp => decode_bmp(bytes, options),
        Profile::Tga => decode_tga(bytes, options),
        Profile::Qoi => decode_qoi(bytes, options),
        Profile::Ppm => decode_ppm(bytes, options),
        Profile::Pam => decode_pam(bytes, options),
        Profile::Png => decode_png(bytes, options),
    }
}

fn encode(profile: Profile, image: &Image, options: &Options) -> Result<Encoded, Error> {
    match profile {
        Profile::Bmp => encode_bmp(image),
        Profile::Tga => encode_tga(image, options.tga_rle),
        Profile::Qoi => encode_qoi(image),
        Profile::Ppm => encode_ppm(image, options.ppm_alpha),
        Profile::Pam => encode_pam(image),
        Profile::Png => encode_png(image),
    }
}

/// Convert one supported Raster Wave A image into another.
///
/// The implementation is dependency-free. It validates the complete source
/// before writing output, so malformed inputs never leave a partial target.
pub fn convert<R: Read + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.max_pixels == 0 { return Err(Error::InvalidOptions("max_pixels must be non-zero")); }
    if options.max_input_bytes == 0 { return Err(Error::InvalidOptions("max_input_bytes must be non-zero")); }
    let take = options.max_input_bytes.saturating_add(1);
    let mut source = Vec::new();
    input.take(take).read_to_end(&mut source)?;
    if source.len() as u64 > options.max_input_bytes {
        return Err(Error::InputTooLarge { bytes: source.len() as u64, limit: options.max_input_bytes });
    }
    let image = decode(SOURCE, &source, options)?;
    let non_opaque_pixels = image.pixels.iter().filter(|pixel| pixel.a != 255).count() as u64;
    let encoded = encode(TARGET, &image, options)?;
    output.write_all(&encoded.bytes)?;
    let pixel_memory = (image.pixels.len() as u64).checked_mul(4).ok_or(Error::IntegerOverflow("working memory"))?;
    let peak = source.len() as u64 + pixel_memory + encoded.bytes.len() as u64;
    Ok(Report {
        input_bytes: source.len() as u64,
        output_bytes: encoded.bytes.len() as u64,
        width: image.width,
        height: image.height,
        pixels: image.pixels.len() as u64,
        source_profile: SOURCE.name(),
        target_profile: TARGET.name(),
        source_channels: image.source_channels,
        target_channels: encoded.channels,
        non_opaque_pixels,
        alpha_action: encoded.alpha_action,
        peak_working_memory_bytes: peak,
        strategy: "rgba8-code-value-exact",
        backend: "native-portable",
        warnings: image.warnings,
    })
}

fn decode_bmp(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    if bytes.get(..2) != Some(b"BM") { return Err(Error::InvalidSignature("BMP")); }
    let header = need(bytes, 0, 54, "BMP headers")?;
    let declared = le_u32(&header[2..6]) as usize;
    let offset = le_u32(&header[10..14]) as usize;
    let dib = le_u32(&header[14..18]) as usize;
    if dib < 40 { return Err(Error::Unsupported("BMP DIB headers smaller than BITMAPINFOHEADER")); }
    need(bytes, 14, dib, "BMP DIB header")?;
    let width_i = le_i32(&header[18..22]);
    let height_i = le_i32(&header[22..26]);
    if width_i <= 0 || height_i == 0 || height_i == i32::MIN { return Err(Error::InvalidHeader("invalid BMP dimensions")); }
    if le_u16(&header[26..28]) != 1 { return Err(Error::InvalidHeader("BMP planes must equal one")); }
    let bpp = le_u16(&header[28..30]);
    let compression = le_u32(&header[30..34]);
    let explicit_alpha = if bpp == 32 && matches!(compression, 3 | 6) {
        if dib < 56 { return Err(Error::Unsupported("external BMP bitfield masks")); }
        let masks = need(bytes, 14 + 40, 16, "BMP channel masks")?;
        let values = (le_u32(&masks[0..4]), le_u32(&masks[4..8]), le_u32(&masks[8..12]), le_u32(&masks[12..16]));
        if values != (0x00ff0000, 0x0000ff00, 0x000000ff, 0xff000000) {
            return Err(Error::Unsupported("BMP 32-bit masks other than BGRA8"));
        }
        true
    } else { false };
    if !((bpp == 24 && compression == 0) || (bpp == 32 && compression == 0) || explicit_alpha) {
        return Err(Error::Unsupported("BMP Wave A accepts 24-bit BI_RGB and 32-bit BI_RGB/BGRA8 bitfields"));
    }
    let width = width_i as u32;
    let height = height_i.unsigned_abs();
    let count = checked_pixels(width, height, options)?;
    let bytes_per_pixel = (bpp / 8) as usize;
    let row_bytes = (width as usize).checked_mul(bytes_per_pixel).ok_or(Error::IntegerOverflow("BMP row"))?;
    let stride = row_bytes.checked_add(3).ok_or(Error::IntegerOverflow("BMP stride"))? & !3;
    let raster_bytes = stride.checked_mul(height as usize).ok_or(Error::IntegerOverflow("BMP raster"))?;
    need(bytes, offset, raster_bytes, "BMP raster")?;
    if declared != 0 {
        if declared > bytes.len() { return Err(Error::Truncated("declared BMP file")); }
        if options.strict_trailing_data && declared < bytes.len() { return Err(Error::TrailingData { bytes: bytes.len() - declared }); }
    } else if options.strict_trailing_data && offset + raster_bytes < bytes.len() {
        return Err(Error::TrailingData { bytes: bytes.len() - offset - raster_bytes });
    }
    let top_down = height_i < 0;
    let mut pixels = vec![Pixel::default(); count];
    for y in 0..height as usize {
        let stored_y = if top_down { y } else { height as usize - 1 - y };
        let row = offset + stored_y * stride;
        for x in 0..width as usize {
            let pos = row + x * bytes_per_pixel;
            let a = if bpp == 32 && (explicit_alpha || options.preserve_unmarked_bmp_alpha) { bytes[pos + 3] } else { 255 };
            pixels[y * width as usize + x] = Pixel { r: bytes[pos + 2], g: bytes[pos + 1], b: bytes[pos], a };
        }
    }
    let mut warnings = Vec::new();
    if bpp == 32 && compression == 0 && !options.preserve_unmarked_bmp_alpha {
        warnings.push("unmarked BI_RGB fourth byte normalized to opaque".into());
    }
    Ok(Image { width, height, source_channels: if explicit_alpha || options.preserve_unmarked_bmp_alpha { 4 } else { 3 }, pixels, warnings })
}

fn encode_bmp(image: &Image) -> Result<Encoded, Error> {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let bpp = if alpha { 32usize } else { 24usize };
    let dib = if alpha { 108usize } else { 40usize };
    let offset = 14usize + dib;
    let pixel_bytes = bpp / 8;
    let row_bytes = (image.width as usize).checked_mul(pixel_bytes).ok_or(Error::IntegerOverflow("BMP row"))?;
    let stride = row_bytes.checked_add(3).ok_or(Error::IntegerOverflow("BMP stride"))? & !3;
    let raster = stride.checked_mul(image.height as usize).ok_or(Error::IntegerOverflow("BMP raster"))?;
    let total = offset.checked_add(raster).ok_or(Error::IntegerOverflow("BMP size"))?;
    let total32 = u32::try_from(total).map_err(|_| Error::Unsupported("BMP output exceeds 4 GiB"))?;
    let raster32 = u32::try_from(raster).map_err(|_| Error::Unsupported("BMP raster exceeds 4 GiB"))?;
    let height = i32::try_from(image.height).map_err(|_| Error::Unsupported("BMP height exceeds i32"))?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&total32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&(dib as u32).to_le_bytes());
    out.extend_from_slice(&(image.width as i32).to_le_bytes());
    out.extend_from_slice(&(-height).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(bpp as u16).to_le_bytes());
    out.extend_from_slice(&(if alpha { 3u32 } else { 0u32 }).to_le_bytes());
    out.extend_from_slice(&raster32.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    if alpha {
        out.extend_from_slice(&0x00ff0000u32.to_le_bytes());
        out.extend_from_slice(&0x0000ff00u32.to_le_bytes());
        out.extend_from_slice(&0x000000ffu32.to_le_bytes());
        out.extend_from_slice(&0xff000000u32.to_le_bytes());
        out.extend_from_slice(&0x73524742u32.to_le_bytes());
        out.resize(offset, 0);
    }
    for row in image.pixels.chunks_exact(image.width as usize) {
        for pixel in row {
            out.extend_from_slice(&[pixel.b, pixel.g, pixel.r]);
            if alpha { out.push(pixel.a); }
        }
        out.resize(out.len() + stride - row_bytes, 0);
    }
    debug_assert_eq!(out.len(), total);
    Ok(Encoded { bytes: out, channels: if alpha { 4 } else { 3 }, alpha_action: "preserved" })
}

fn decode_tga(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let header = need(bytes, 0, 18, "TGA header")?;
    let id_length = header[0] as usize;
    if header[1] != 0 { return Err(Error::Unsupported("color-mapped TGA")); }
    let image_type = header[2];
    if !matches!(image_type, 2 | 3 | 10 | 11) { return Err(Error::Unsupported("TGA image type outside raw/RLE truecolor/grayscale")); }
    let width = le_u16(&header[12..14]) as u32;
    let height = le_u16(&header[14..16]) as u32;
    let depth = header[16];
    let truecolor = matches!(image_type, 2 | 10);
    if (truecolor && !matches!(depth, 24 | 32)) || (!truecolor && !matches!(depth, 8 | 16)) {
        return Err(Error::Unsupported("TGA pixel depth"));
    }
    let count = checked_pixels(width, height, options)?;
    let pixel_bytes = (depth / 8) as usize;
    let mut pos = 18usize.checked_add(id_length).ok_or(Error::IntegerOverflow("TGA image offset"))?;
    need(bytes, 18, id_length, "TGA image ID")?;
    let mut file_pixels = Vec::with_capacity(count);
    let decode_pixel = |sample: &[u8]| -> Pixel {
        if truecolor {
            Pixel { r: sample[2], g: sample[1], b: sample[0], a: if pixel_bytes == 4 { sample[3] } else { 255 } }
        } else {
            Pixel { r: sample[0], g: sample[0], b: sample[0], a: if pixel_bytes == 2 { sample[1] } else { 255 } }
        }
    };
    if matches!(image_type, 2 | 3) {
        let raw = need(bytes, pos, count.checked_mul(pixel_bytes).ok_or(Error::IntegerOverflow("TGA raster"))?, "TGA raster")?;
        for sample in raw.chunks_exact(pixel_bytes) { file_pixels.push(decode_pixel(sample)); }
        pos += raw.len();
    } else {
        while file_pixels.len() < count {
            let packet = *bytes.get(pos).ok_or(Error::Truncated("TGA RLE packet"))?;
            pos += 1;
            let length = (packet as usize & 0x7f) + 1;
            if file_pixels.len().checked_add(length).ok_or(Error::IntegerOverflow("TGA RLE pixels"))? > count {
                return Err(Error::InvalidHeader("TGA RLE packet exceeds image dimensions"));
            }
            if packet & 0x80 != 0 {
                let sample = need(bytes, pos, pixel_bytes, "TGA RLE sample")?;
                pos += pixel_bytes;
                file_pixels.extend(std::iter::repeat_n(decode_pixel(sample), length));
            } else {
                let raw_bytes = length.checked_mul(pixel_bytes).ok_or(Error::IntegerOverflow("TGA raw packet"))?;
                let raw = need(bytes, pos, raw_bytes, "TGA raw packet")?;
                pos += raw_bytes;
                for sample in raw.chunks_exact(pixel_bytes) { file_pixels.push(decode_pixel(sample)); }
            }
        }
    }
    let top = header[17] & 0x20 != 0;
    let right = header[17] & 0x10 != 0;
    let mut pixels = vec![Pixel::default(); count];
    for (index, pixel) in file_pixels.into_iter().enumerate() {
        let file_y = index / width as usize;
        let file_x = index % width as usize;
        let y = if top { file_y } else { height as usize - 1 - file_y };
        let x = if right { width as usize - 1 - file_x } else { file_x };
        pixels[y * width as usize + x] = pixel;
    }
    if options.strict_trailing_data && pos < bytes.len() {
        let tail = &bytes[pos..];
        let valid_footer = tail.len() >= 26 && &tail[tail.len() - 18..] == b"TRUEVISION-XFILE.\0";
        if !valid_footer { return Err(Error::TrailingData { bytes: bytes.len() - pos }); }
    }
    Ok(Image { width, height, source_channels: if matches!(depth, 16 | 32) { 4 } else { 3 }, pixels, warnings: Vec::new() })
}

fn push_tga_pixel(out: &mut Vec<u8>, pixel: Pixel, alpha: bool) {
    out.extend_from_slice(&[pixel.b, pixel.g, pixel.r]);
    if alpha { out.push(pixel.a); }
}

fn encode_tga(image: &Image, rle: bool) -> Result<Encoded, Error> {
    let width = u16::try_from(image.width).map_err(|_| Error::Unsupported("TGA width exceeds 65535"))?;
    let height = u16::try_from(image.height).map_err(|_| Error::Unsupported("TGA height exceeds 65535"))?;
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, if rle { 10 } else { 2 }]);
    out.extend_from_slice(&[0; 9]);
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.push(if alpha { 32 } else { 24 });
    out.push(0x20 | if alpha { 8 } else { 0 });
    if !rle {
        for &pixel in &image.pixels { push_tga_pixel(&mut out, pixel, alpha); }
    } else {
        for row in image.pixels.chunks_exact(image.width as usize) {
            let mut x = 0usize;
            while x < row.len() {
                let mut run = 1usize;
                while run < 128 && x + run < row.len() && row[x + run] == row[x] { run += 1; }
                if run >= 2 {
                    out.push(0x80 | (run as u8 - 1));
                    push_tga_pixel(&mut out, row[x], alpha);
                    x += run;
                } else {
                    let start = x;
                    x += 1;
                    while x < row.len() && x - start < 128 {
                        let mut next_run = 1usize;
                        while next_run < 2 && x + next_run < row.len() && row[x + next_run] == row[x] { next_run += 1; }
                        if next_run >= 2 { break; }
                        x += 1;
                    }
                    out.push((x - start) as u8 - 1);
                    for &pixel in &row[start..x] { push_tga_pixel(&mut out, pixel, alpha); }
                }
            }
        }
    }
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(b"TRUEVISION-XFILE.\0");
    Ok(Encoded { bytes: out, channels: if alpha { 4 } else { 3 }, alpha_action: "preserved" })
}

fn qoi_hash(pixel: Pixel) -> usize {
    (pixel.r as usize * 3 + pixel.g as usize * 5 + pixel.b as usize * 7 + pixel.a as usize * 11) % 64
}

fn decode_qoi(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let header = need(bytes, 0, 14, "QOI header")?;
    if &header[..4] != b"qoif" { return Err(Error::InvalidSignature("QOI")); }
    let width = be_u32(&header[4..8]);
    let height = be_u32(&header[8..12]);
    let channels = header[12];
    if !matches!(channels, 3 | 4) { return Err(Error::InvalidHeader("QOI channels must be 3 or 4")); }
    if header[13] > 1 { return Err(Error::InvalidHeader("QOI colorspace must be 0 or 1")); }
    let count = checked_pixels(width, height, options)?;
    let mut pos = 14usize;
    let mut previous = Pixel { r: 0, g: 0, b: 0, a: 255 };
    let mut index = [Pixel::default(); 64];
    let mut run = 0usize;
    let mut pixels = Vec::with_capacity(count);
    while pixels.len() < count {
        if run > 0 {
            run -= 1;
        } else {
            let tag = *bytes.get(pos).ok_or(Error::Truncated("QOI chunk"))?;
            pos += 1;
            if tag == 0xfe {
                let rgb = need(bytes, pos, 3, "QOI RGB chunk")?;
                pos += 3;
                previous.r = rgb[0]; previous.g = rgb[1]; previous.b = rgb[2];
            } else if tag == 0xff {
                let rgba = need(bytes, pos, 4, "QOI RGBA chunk")?;
                pos += 4;
                previous = Pixel { r: rgba[0], g: rgba[1], b: rgba[2], a: rgba[3] };
            } else {
                match tag >> 6 {
                    0 => previous = index[(tag & 0x3f) as usize],
                    1 => {
                        previous.r = previous.r.wrapping_add(((tag >> 4) & 3).wrapping_sub(2));
                        previous.g = previous.g.wrapping_add(((tag >> 2) & 3).wrapping_sub(2));
                        previous.b = previous.b.wrapping_add((tag & 3).wrapping_sub(2));
                    }
                    2 => {
                        let second = *bytes.get(pos).ok_or(Error::Truncated("QOI LUMA chunk"))?;
                        pos += 1;
                        let dg = (tag & 0x3f).wrapping_sub(32);
                        previous.r = previous.r.wrapping_add(dg).wrapping_add((second >> 4).wrapping_sub(8));
                        previous.g = previous.g.wrapping_add(dg);
                        previous.b = previous.b.wrapping_add(dg).wrapping_add((second & 0x0f).wrapping_sub(8));
                    }
                    3 => run = (tag & 0x3f) as usize,
                    _ => unreachable!(),
                }
            }
        }
        index[qoi_hash(previous)] = previous;
        pixels.push(previous);
    }
    let marker = need(bytes, pos, 8, "QOI end marker")?;
    if marker != [0, 0, 0, 0, 0, 0, 0, 1] { return Err(Error::InvalidHeader("invalid QOI end marker")); }
    pos += 8;
    if options.strict_trailing_data && pos != bytes.len() { return Err(Error::TrailingData { bytes: bytes.len() - pos }); }
    Ok(Image { width, height, source_channels: channels, pixels, warnings: Vec::new() })
}

fn encode_qoi(image: &Image) -> Result<Encoded, Error> {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let mut out = Vec::new();
    out.extend_from_slice(b"qoif");
    out.extend_from_slice(&image.width.to_be_bytes());
    out.extend_from_slice(&image.height.to_be_bytes());
    out.push(if alpha { 4 } else { 3 });
    out.push(0);
    let mut previous = Pixel { r: 0, g: 0, b: 0, a: 255 };
    let mut index = [Pixel::default(); 64];
    let mut run = 0usize;
    for (position, &pixel) in image.pixels.iter().enumerate() {
        if pixel == previous {
            run += 1;
            if run == 62 || position + 1 == image.pixels.len() {
                out.push(0xc0 | (run as u8 - 1));
                run = 0;
            }
            continue;
        }
        if run > 0 {
            out.push(0xc0 | (run as u8 - 1));
            run = 0;
        }
        let hash = qoi_hash(pixel);
        if index[hash] == pixel {
            out.push(hash as u8);
        } else {
            index[hash] = pixel;
            if pixel.a == previous.a {
                let dr = pixel.r.wrapping_sub(previous.r) as i8;
                let dg = pixel.g.wrapping_sub(previous.g) as i8;
                let db = pixel.b.wrapping_sub(previous.b) as i8;
                let dr_dg = dr.wrapping_sub(dg);
                let db_dg = db.wrapping_sub(dg);
                if (-2..=1).contains(&dr) && (-2..=1).contains(&dg) && (-2..=1).contains(&db) {
                    out.push(0x40 | ((dr + 2) as u8) << 4 | ((dg + 2) as u8) << 2 | (db + 2) as u8);
                } else if (-32..=31).contains(&dg) && (-8..=7).contains(&dr_dg) && (-8..=7).contains(&db_dg) {
                    out.push(0x80 | (dg + 32) as u8);
                    out.push(((dr_dg + 8) as u8) << 4 | (db_dg + 8) as u8);
                } else {
                    out.extend_from_slice(&[0xfe, pixel.r, pixel.g, pixel.b]);
                }
            } else {
                out.extend_from_slice(&[0xff, pixel.r, pixel.g, pixel.b, pixel.a]);
            }
        }
        previous = pixel;
    }
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    Ok(Encoded { bytes: out, channels: if alpha { 4 } else { 3 }, alpha_action: "preserved" })
}

struct TokenCursor<'a> { bytes: &'a [u8], pos: usize }

impl<'a> TokenCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, pos: 0 } }
    fn skip(&mut self) {
        loop {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() { self.pos += 1; }
            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'#' {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' { self.pos += 1; }
                continue;
            }
            break;
        }
    }
    fn token(&mut self, region: &'static str) -> Result<&'a [u8], Error> {
        self.skip();
        let start = self.pos;
        while self.pos < self.bytes.len() && !self.bytes[self.pos].is_ascii_whitespace() && self.bytes[self.pos] != b'#' { self.pos += 1; }
        if start == self.pos { return Err(Error::Truncated(region)); }
        Ok(&self.bytes[start..self.pos])
    }
    fn number(&mut self, region: &'static str) -> Result<u32, Error> {
        let token = self.token(region)?;
        let text = std::str::from_utf8(token).map_err(|_| Error::InvalidHeader("non-ASCII Netpbm number"))?;
        text.parse().map_err(|_| Error::InvalidHeader("invalid Netpbm number"))
    }
}

fn scale_sample(value: u16, maxval: u16) -> u8 {
    ((value as u32 * 255 + maxval as u32 / 2) / maxval as u32) as u8
}

fn decode_ppm(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let mut cursor = TokenCursor::new(bytes);
    let magic = cursor.token("PPM magic")?;
    if !matches!(magic, b"P6" | b"P3") { return Err(Error::InvalidSignature("PPM")); }
    let width = cursor.number("PPM width")?;
    let height = cursor.number("PPM height")?;
    let maxval_u32 = cursor.number("PPM maxval")?;
    if maxval_u32 == 0 || maxval_u32 > 65535 { return Err(Error::InvalidHeader("PPM maxval must be 1..65535")); }
    let maxval = maxval_u32 as u16;
    if maxval != 255 && !options.allow_sample_scaling {
        return Err(Error::Unsupported("PPM MAXVAL other than 255 requires allow_sample_scaling"));
    }
    let count = checked_pixels(width, height, options)?;
    let samples = count.checked_mul(3).ok_or(Error::IntegerOverflow("PPM samples"))?;
    let mut pixels = Vec::with_capacity(count);
    if magic == b"P3" {
        for _ in 0..count {
            let r = cursor.number("PPM red sample")?;
            let g = cursor.number("PPM green sample")?;
            let b = cursor.number("PPM blue sample")?;
            if r > maxval_u32 || g > maxval_u32 || b > maxval_u32 { return Err(Error::InvalidHeader("PPM sample exceeds maxval")); }
            pixels.push(Pixel { r: scale_sample(r as u16, maxval), g: scale_sample(g as u16, maxval), b: scale_sample(b as u16, maxval), a: 255 });
        }
        cursor.skip();
        if options.strict_trailing_data && cursor.pos != bytes.len() { return Err(Error::TrailingData { bytes: bytes.len() - cursor.pos }); }
    } else {
        let separator = *bytes.get(cursor.pos).ok_or(Error::Truncated("PPM raster separator"))?;
        if !separator.is_ascii_whitespace() { return Err(Error::InvalidHeader("PPM maxval lacks raster separator")); }
        cursor.pos += 1;
        if separator == b'\r' && bytes.get(cursor.pos) == Some(&b'\n') { cursor.pos += 1; }
        let sample_bytes = if maxval < 256 { 1usize } else { 2usize };
        let raster_len = samples.checked_mul(sample_bytes).ok_or(Error::IntegerOverflow("PPM raster"))?;
        let raster = need(bytes, cursor.pos, raster_len, "PPM raster")?;
        let mut values = Vec::with_capacity(samples);
        if sample_bytes == 1 {
            for &value in raster {
                if value as u16 > maxval { return Err(Error::InvalidHeader("PPM sample exceeds maxval")); }
                values.push(scale_sample(value as u16, maxval));
            }
        } else {
            for pair in raster.chunks_exact(2) {
                let value = u16::from_be_bytes([pair[0], pair[1]]);
                if value > maxval { return Err(Error::InvalidHeader("PPM sample exceeds maxval")); }
                values.push(scale_sample(value, maxval));
            }
        }
        for rgb in values.chunks_exact(3) { pixels.push(Pixel { r: rgb[0], g: rgb[1], b: rgb[2], a: 255 }); }
        cursor.pos += raster_len;
        if options.strict_trailing_data && cursor.pos != bytes.len() { return Err(Error::TrailingData { bytes: bytes.len() - cursor.pos }); }
    }
    let mut warnings = Vec::new();
    if maxval != 255 { warnings.push(format!("PPM maxval {maxval} normalized to 8-bit code values")); }
    Ok(Image { width, height, source_channels: 3, pixels, warnings })
}

fn encode_ppm(image: &Image, policy: AlphaPolicy) -> Result<Encoded, Error> {
    let non_opaque = image.pixels.iter().filter(|pixel| pixel.a != 255).count() as u64;
    if non_opaque > 0 && policy == AlphaPolicy::Reject { return Err(Error::AlphaNotRepresentable { non_opaque_pixels: non_opaque }); }
    let mut out = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
    for pixel in &image.pixels {
        match policy {
            AlphaPolicy::Reject | AlphaPolicy::Discard => out.extend_from_slice(&[pixel.r, pixel.g, pixel.b]),
            AlphaPolicy::CompositeBlack => {
                let a = pixel.a as u16;
                out.extend_from_slice(&[
                    ((pixel.r as u16 * a + 127) / 255) as u8,
                    ((pixel.g as u16 * a + 127) / 255) as u8,
                    ((pixel.b as u16 * a + 127) / 255) as u8,
                ]);
            }
        }
    }
    let action = if non_opaque == 0 { "not-present" } else if policy == AlphaPolicy::Discard { "discarded-explicitly" } else { "composited-black-explicitly" };
    Ok(Encoded { bytes: out, channels: 3, alpha_action: action })
}

fn decode_pam(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    if !bytes.starts_with(b"P7\n") { return Err(Error::InvalidSignature("PAM")); }
    let mut pos = 3usize;
    let mut width = None;
    let mut height = None;
    let mut depth = None;
    let mut maxval = None;
    let mut tuple_types = Vec::new();
    loop {
        let relative = bytes.get(pos..).ok_or(Error::Truncated("PAM header"))?;
        let line_end = relative.iter().position(|&byte| byte == b'\n').ok_or(Error::Truncated("PAM header line"))?;
        let line = &relative[..line_end];
        pos += line_end + 1;
        let text = std::str::from_utf8(line).map_err(|_| Error::InvalidHeader("non-ASCII PAM header"))?.trim();
        if text.is_empty() || text.starts_with('#') { continue; }
        if text == "ENDHDR" { break; }
        let mut parts = text.split_ascii_whitespace();
        let key = parts.next().ok_or(Error::InvalidHeader("empty PAM header field"))?;
        let rest = parts.collect::<Vec<_>>();
        match key {
            "WIDTH" => { if width.is_some() || rest.len() != 1 { return Err(Error::InvalidHeader("duplicate or invalid PAM WIDTH")); } width = Some(rest[0].parse::<u32>().map_err(|_| Error::InvalidHeader("invalid PAM WIDTH"))?); }
            "HEIGHT" => { if height.is_some() || rest.len() != 1 { return Err(Error::InvalidHeader("duplicate or invalid PAM HEIGHT")); } height = Some(rest[0].parse::<u32>().map_err(|_| Error::InvalidHeader("invalid PAM HEIGHT"))?); }
            "DEPTH" => { if depth.is_some() || rest.len() != 1 { return Err(Error::InvalidHeader("duplicate or invalid PAM DEPTH")); } depth = Some(rest[0].parse::<u8>().map_err(|_| Error::InvalidHeader("invalid PAM DEPTH"))?); }
            "MAXVAL" => { if maxval.is_some() || rest.len() != 1 { return Err(Error::InvalidHeader("duplicate or invalid PAM MAXVAL")); } maxval = Some(rest[0].parse::<u16>().map_err(|_| Error::InvalidHeader("invalid PAM MAXVAL"))?); }
            "TUPLTYPE" => { if rest.is_empty() { return Err(Error::InvalidHeader("empty PAM TUPLTYPE")); } tuple_types.push(rest.join(" ")); }
            _ => return Err(Error::Unsupported("unknown PAM header field")),
        }
    }
    let width = width.ok_or(Error::InvalidHeader("missing PAM WIDTH"))?;
    let height = height.ok_or(Error::InvalidHeader("missing PAM HEIGHT"))?;
    let depth = depth.ok_or(Error::InvalidHeader("missing PAM DEPTH"))?;
    let maxval = maxval.ok_or(Error::InvalidHeader("missing PAM MAXVAL"))?;
    if !matches!(depth, 1 | 2 | 3 | 4) { return Err(Error::Unsupported("PAM depth outside 1..4 visual tuples")); }
    if maxval == 0 { return Err(Error::InvalidHeader("PAM MAXVAL must be non-zero")); }
    if maxval != 255 && !options.allow_sample_scaling {
        return Err(Error::Unsupported("PAM MAXVAL other than 255 requires allow_sample_scaling"));
    }
    if !tuple_types.is_empty() {
        let tuple = tuple_types.join(" ");
        let allowed = matches!((depth, tuple.as_str()), (1, "BLACKANDWHITE") | (1, "GRAYSCALE") | (2, "GRAYSCALE_ALPHA") | (3, "RGB") | (4, "RGB_ALPHA"));
        if !allowed { return Err(Error::Unsupported("PAM tuple type/depth combination")); }
    }
    let count = checked_pixels(width, height, options)?;
    let sample_bytes = if maxval < 256 { 1usize } else { 2usize };
    let raster_len = count.checked_mul(depth as usize).and_then(|v| v.checked_mul(sample_bytes)).ok_or(Error::IntegerOverflow("PAM raster"))?;
    let raster = need(bytes, pos, raster_len, "PAM raster")?;
    let mut samples = Vec::with_capacity(count * depth as usize);
    if sample_bytes == 1 {
        for &value in raster {
            if value as u16 > maxval { return Err(Error::InvalidHeader("PAM sample exceeds MAXVAL")); }
            samples.push(scale_sample(value as u16, maxval));
        }
    } else {
        for pair in raster.chunks_exact(2) {
            let value = u16::from_be_bytes([pair[0], pair[1]]);
            if value > maxval { return Err(Error::InvalidHeader("PAM sample exceeds MAXVAL")); }
            samples.push(scale_sample(value, maxval));
        }
    }
    let mut pixels = Vec::with_capacity(count);
    for tuple in samples.chunks_exact(depth as usize) {
        pixels.push(match depth {
            1 => Pixel { r: tuple[0], g: tuple[0], b: tuple[0], a: 255 },
            2 => Pixel { r: tuple[0], g: tuple[0], b: tuple[0], a: tuple[1] },
            3 => Pixel { r: tuple[0], g: tuple[1], b: tuple[2], a: 255 },
            4 => Pixel { r: tuple[0], g: tuple[1], b: tuple[2], a: tuple[3] },
            _ => unreachable!(),
        });
    }
    pos += raster_len;
    if options.strict_trailing_data && pos != bytes.len() { return Err(Error::TrailingData { bytes: bytes.len() - pos }); }
    let mut warnings = Vec::new();
    if maxval != 255 { warnings.push(format!("PAM maxval {maxval} normalized to 8-bit code values")); }
    Ok(Image { width, height, source_channels: depth, pixels, warnings })
}

fn encode_pam(image: &Image) -> Result<Encoded, Error> {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let depth = if alpha { 4 } else { 3 };
    let tuple = if alpha { "RGB_ALPHA" } else { "RGB" };
    let mut out = format!("P7\nWIDTH {}\nHEIGHT {}\nDEPTH {depth}\nMAXVAL 255\nTUPLTYPE {tuple}\nENDHDR\n", image.width, image.height).into_bytes();
    for pixel in &image.pixels {
        out.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
        if alpha { out.push(pixel.a); }
    }
    Ok(Encoded { bytes: out, channels: depth, alpha_action: "preserved" })
}

fn fixture_image(alpha: bool, width: u32, height: u32) -> Image {
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push(Pixel {
                r: x.wrapping_mul(37).wrapping_add(y.wrapping_mul(11)) as u8,
                g: x.wrapping_mul(3).wrapping_add(y.wrapping_mul(29)) as u8,
                b: x.wrapping_mul(17).wrapping_add(y.wrapping_mul(5)) as u8,
                a: if alpha && (x + y) % 3 == 0 { 96 } else { 255 },
            });
        }
    }
    Image { width, height, source_channels: if alpha { 4 } else { 3 }, pixels, warnings: Vec::new() }
}

/// Deterministic valid source fixture used by standalone, Adapter and copy-out tests.
pub fn conformance_fixture() -> Vec<u8> {
    encode(SOURCE, &fixture_image(false, 8, 4), &Options::default()).expect("internal fixture encoder").bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_conversion_preserves_opaque_pixels() {
        let original = fixture_image(false, 8, 4);
        let source = encode(SOURCE, &original, &Options::default()).unwrap().bytes;
        let mut output = Vec::new();
        let report = convert(&mut &source[..], &mut output, &Options::default()).unwrap();
        let decoded = decode(TARGET, &output, &Options::default()).unwrap();
        assert_eq!(decoded.width, original.width);
        assert_eq!(decoded.height, original.height);
        assert_eq!(decoded.pixels, original.pixels);
        assert_eq!(report.pixels, 32);
        assert_eq!(report.strategy, "rgba8-code-value-exact");
    }

    #[test]
    fn alpha_contract_is_explicit() {
        let original = fixture_image(true, 3, 2);
        let source = encode(SOURCE, &original, &Options { ppm_alpha: AlphaPolicy::Discard, ..Options::default() }).unwrap().bytes;
        let source_decoded = decode(SOURCE, &source, &Options::default()).unwrap();
        let mut output = Vec::new();
        let result = convert(&mut &source[..], &mut output, &Options::default());
        if TARGET == Profile::Ppm && source_decoded.pixels.iter().any(|pixel| pixel.a != 255) {
            assert!(matches!(result, Err(Error::AlphaNotRepresentable { .. })));
        } else {
            let report = result.unwrap();
            let decoded = decode(TARGET, &output, &Options::default()).unwrap();
            assert_eq!(decoded.pixels, source_decoded.pixels);
            assert_eq!(report.alpha_action, "preserved");
        }
    }

    #[test]
    fn malformed_signature_is_rejected() {
        let error = convert(&mut &b"not-an-image"[..], &mut Vec::new(), &Options::default()).unwrap_err();
        assert!(matches!(error, Error::InvalidSignature(_) | Error::Truncated(_) | Error::Png(_)));
    }

    #[test]
    fn pixel_limit_is_enforced_before_allocation() {
        let source = conformance_fixture();
        let options = Options { max_pixels: 1, ..Options::default() };
        let error = convert(&mut &source[..], &mut Vec::new(), &options).unwrap_err();
        assert!(matches!(error, Error::PixelLimitExceeded { .. }));
    }
}

fn decode_png(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let decoded = png_native::decode(bytes, &png_native::DecodeOptions {
        max_pixels: options.max_pixels,
        max_inflate_bytes: options.max_input_bytes,
        strict_crc: true,
        strict_trailing_data: options.strict_trailing_data,
    }).map_err(|error| match error {
        png_native::Error::Limit("pixel count") => Error::PixelLimitExceeded {
            pixels: options.max_pixels.saturating_add(1),
            limit: options.max_pixels,
        },
        other => Error::Png(other.to_string()),
    })?;
    if decoded.source_bit_depth == 16 && !options.allow_sample_scaling {
        return Err(Error::Unsupported("16-bit PNG to an 8-bit carrier requires allow_sample_scaling"));
    }
    let mut warnings = decoded.warnings;
    if decoded.source_bit_depth == 16 {
        warnings.push("PNG 16-bit samples explicitly scaled to 8-bit code values".into());
    }
    let pixels = decoded.pixels.into_iter().map(|pixel| Pixel {
        r: ((pixel.r as u32 + 128) / 257) as u8,
        g: ((pixel.g as u32 + 128) / 257) as u8,
        b: ((pixel.b as u32 + 128) / 257) as u8,
        a: ((pixel.a as u32 + 128) / 257) as u8,
    }).collect();
    Ok(Image { width: decoded.width, height: decoded.height, source_channels: decoded.source_channels, pixels, warnings })
}

fn encode_png(image: &Image) -> Result<Encoded, Error> {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let native = png_native::Image {
        width: image.width,
        height: image.height,
        source_channels: if alpha { 4 } else { 3 },
        source_bit_depth: 8,
        source_color_type: if alpha { 6 } else { 2 },
        interlaced: false,
        pixels: image.pixels.iter().map(|pixel| png_native::Pixel16 {
            r: pixel.r as u16 * 257,
            g: pixel.g as u16 * 257,
            b: pixel.b as u16 * 257,
            a: pixel.a as u16 * 257,
        }).collect(),
        warnings: Vec::new(),
    };
    let bytes = png_native::encode(&native, png_native::Filter::Adaptive).map_err(|error| Error::Png(error.to_string()))?;
    Ok(Encoded { bytes, channels: if alpha { 4 } else { 3 }, alpha_action: "preserved" })
}
