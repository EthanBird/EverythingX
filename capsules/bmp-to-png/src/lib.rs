#![forbid(unsafe_code)]

mod bmp;
mod png;
mod zlib;

use std::fmt;
use std::io::{self, Read, Seek, Write};

const DEFAULT_IDAT_CHUNK_SIZE: usize = 64 * 1024;
const MAX_IDAT_CHUNK_SIZE: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterStrategy {
    None,
    Sub,
    Up,
    Average,
    Paeth,
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// Fixed Huffman Deflate with a streaming distance-one run detector.
    FixedRle,
    /// Stored Deflate blocks. This minimizes CPU work at the cost of output size.
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmarkedAlpha {
    /// Treat reserved palette bytes and BI_RGB's fourth byte as opaque.
    Opaque,
    /// Preserve those bytes as alpha even when no alpha mask declares them.
    Preserve,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub filter: FilterStrategy,
    pub compression: CompressionStrategy,
    pub unmarked_alpha: UnmarkedAlpha,
    pub idat_chunk_size: usize,
    pub max_pixels: u64,
    pub strict_declared_file_size: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            filter: FilterStrategy::Adaptive,
            compression: CompressionStrategy::FixedRle,
            unmarked_alpha: UnmarkedAlpha::Opaque,
            idat_chunk_size: DEFAULT_IDAT_CHUNK_SIZE,
            max_pixels: DEFAULT_MAX_PIXELS,
            strict_declared_file_size: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub source_bits_per_pixel: u16,
    pub source_compression: &'static str,
    pub source_top_down: bool,
    pub palette_entries: u32,
    pub output_color_type: &'static str,
    pub alpha_preserved: bool,
    pub decoded_pixels: u64,
    pub peak_working_memory_bytes: u64,
    pub filter: &'static str,
    pub compression: &'static str,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    InvalidSignature,
    UnsupportedDibHeader(u32),
    InvalidDimensions { width: i32, height: i32 },
    PixelLimitExceeded { pixels: u64, limit: u64 },
    UnsupportedPlanes(u16),
    UnsupportedBitDepth(u16),
    UnsupportedCompression { compression: u32, bits_per_pixel: u16 },
    InvalidPixelOffset(u32),
    DeclaredFileSizeExceedsInput { declared: u32, actual: u64 },
    Truncated(&'static str),
    InvalidPalette(&'static str),
    PaletteIndexOutOfRange { index: u8, entries: usize },
    InvalidBitfieldMasks(&'static str),
    InvalidRle(&'static str),
    IntegerOverflow(&'static str),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(formatter, "invalid options: {message}"),
            Self::InvalidSignature => write!(formatter, "input does not begin with the BMP BM signature"),
            Self::UnsupportedDibHeader(size) => write!(formatter, "unsupported BMP DIB header size {size}"),
            Self::InvalidDimensions { width, height } => {
                write!(formatter, "invalid BMP dimensions {width}x{height}")
            }
            Self::PixelLimitExceeded { pixels, limit } => {
                write!(formatter, "BMP contains {pixels} pixels, exceeding limit {limit}")
            }
            Self::UnsupportedPlanes(planes) => write!(formatter, "BMP planes must be 1, found {planes}"),
            Self::UnsupportedBitDepth(depth) => write!(formatter, "unsupported BMP bit depth {depth}"),
            Self::UnsupportedCompression { compression, bits_per_pixel } => {
                write!(formatter, "unsupported BMP compression {compression} for {bits_per_pixel} bpp")
            }
            Self::InvalidPixelOffset(offset) => write!(formatter, "invalid BMP pixel offset {offset}"),
            Self::DeclaredFileSizeExceedsInput { declared, actual } => {
                write!(formatter, "BMP declares {declared} bytes but input contains {actual}")
            }
            Self::Truncated(region) => write!(formatter, "truncated BMP {region}"),
            Self::InvalidPalette(message) => write!(formatter, "invalid BMP palette: {message}"),
            Self::PaletteIndexOutOfRange { index, entries } => {
                write!(formatter, "palette index {index} exceeds {entries} entries")
            }
            Self::InvalidBitfieldMasks(message) => write!(formatter, "invalid BMP bitfield masks: {message}"),
            Self::InvalidRle(message) => write!(formatter, "invalid BMP RLE stream: {message}"),
            Self::IntegerOverflow(context) => write!(formatter, "integer overflow while computing {context}"),
            Self::Io(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn filter_name(value: FilterStrategy) -> &'static str {
    match value {
        FilterStrategy::None => "none",
        FilterStrategy::Sub => "sub",
        FilterStrategy::Up => "up",
        FilterStrategy::Average => "average",
        FilterStrategy::Paeth => "paeth",
        FilterStrategy::Adaptive => "adaptive",
    }
}

fn compression_name(value: CompressionStrategy) -> &'static str {
    match value {
        CompressionStrategy::FixedRle => "fixed-rle",
        CompressionStrategy::Store => "store",
    }
}

/// Converts one Windows BMP byte stream into one PNG byte stream.
///
/// The reader must be seekable because bottom-up BMP rows have to be emitted
/// to PNG in top-down order. The function writes PNG incrementally and never
/// holds the complete decoded image unless the source uses BMP RLE4/RLE8.
pub fn convert<R: Read + Seek + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.idat_chunk_size == 0 {
        return Err(Error::InvalidOptions("idat_chunk_size must be non-zero"));
    }
    if options.idat_chunk_size > MAX_IDAT_CHUNK_SIZE {
        return Err(Error::InvalidOptions("idat_chunk_size exceeds 16 MiB"));
    }
    if options.max_pixels == 0 {
        return Err(Error::InvalidOptions("max_pixels must be non-zero"));
    }

    let image = bmp::inspect(input, options)?;
    let stats = png::encode(input, output, &image, options)?;
    let row_bytes = (image.width as u64)
        .checked_mul(image.output_channels as u64)
        .ok_or(Error::IntegerOverflow("decoded row size"))?;
    let filter_memory = row_bytes
        .checked_mul(3)
        .and_then(|value| value.checked_add(options.idat_chunk_size as u64))
        .ok_or(Error::IntegerOverflow("working memory estimate"))?;
    let rle_memory = image
        .rle_pixels
        .as_ref()
        .map_or(0, |pixels| pixels.len() as u64);

    Ok(Report {
        input_bytes: image.file_size,
        output_bytes: stats.output_bytes,
        width: image.width,
        height: image.height,
        source_bits_per_pixel: image.bits_per_pixel,
        source_compression: image.compression.name(),
        source_top_down: image.top_down,
        palette_entries: image.palette.len() as u32,
        output_color_type: if image.output_channels == 4 { "rgba8" } else { "rgb8" },
        alpha_preserved: image.output_channels == 4,
        decoded_pixels: (image.width as u64) * (image.height as u64),
        peak_working_memory_bytes: filter_memory + rle_memory,
        filter: filter_name(options.filter),
        compression: compression_name(options.compression),
        strategy: "pixel-exact",
        backend: "native-portable",
        warnings: image.warnings,
    })
}

