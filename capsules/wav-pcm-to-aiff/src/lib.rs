#![forbid(unsafe_code)]

mod aiff;
mod wave;

use std::fmt;
use std::io::{self, Read, Seek, Write};

const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;
const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;
const DEFAULT_METADATA_LIMIT: u64 = 1024 * 1024;
const DEFAULT_MAX_CHANNELS: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataPolicy {
    /// Map common RIFF INFO fields to AIFF NAME/AUTH/ANNO/(c) chunks.
    CommonText,
    /// Do not emit source metadata chunks.
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub metadata: MetadataPolicy,
    pub strict_header_consistency: bool,
    pub buffer_size: usize,
    pub max_metadata_bytes: u64,
    pub max_channels: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            metadata: MetadataPolicy::CommonText,
            strict_header_consistency: true,
            buffer_size: DEFAULT_BUFFER_SIZE,
            max_metadata_bytes: DEFAULT_METADATA_LIMIT,
            max_channels: DEFAULT_MAX_CHANNELS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub channels: u16,
    pub sample_rate: u32,
    pub container_bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    pub sample_frames: u32,
    pub source_audio_bytes: u64,
    pub output_audio_bytes: u64,
    pub source_data_chunks: u32,
    pub wave_format_extensible: bool,
    pub metadata_chunks_found: u32,
    pub metadata_chunks_preserved: u32,
    pub metadata_bytes_preserved: u64,
    pub peak_working_memory_bytes: u64,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    InvalidSignature([u8; 4]),
    UnsupportedRf64,
    UnsupportedRifx,
    InvalidWaveForm,
    Truncated(&'static str),
    MissingFormatChunk,
    MissingDataChunk,
    DuplicateFormatChunk,
    UnsupportedFormatTag(u16),
    InvalidExtensibleFormat(&'static str),
    UnsupportedChannels { channels: u16, limit: u16 },
    UnsupportedBitsPerSample(u16),
    InvalidSampleRate(u32),
    InvalidBlockAlign { declared: u16, expected: u16 },
    InvalidByteRate { declared: u32, expected: u32 },
    PartialSampleFrame { data_bytes: u64, block_align: u16 },
    MetadataLimitExceeded { bytes: u64, limit: u64 },
    DeclaredRiffSizeExceedsInput { declared_end: u64, actual: u64 },
    AiffSizeLimitExceeded { bytes: u64 },
    IntegerOverflow(&'static str),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(formatter, "invalid options: {message}"),
            Self::InvalidSignature(signature) => write!(
                formatter,
                "unsupported WAV container signature {:?}",
                String::from_utf8_lossy(signature)
            ),
            Self::UnsupportedRf64 => write!(formatter, "RF64 input cannot be represented by classic AIFF 32-bit chunk sizes"),
            Self::UnsupportedRifx => write!(formatter, "big-endian RIFX/WAVE input is not supported by this Capsule version"),
            Self::InvalidWaveForm => write!(formatter, "RIFF form type is not WAVE"),
            Self::Truncated(region) => write!(formatter, "truncated WAV {region}"),
            Self::MissingFormatChunk => write!(formatter, "WAV has no fmt chunk"),
            Self::MissingDataChunk => write!(formatter, "WAV has no data chunk"),
            Self::DuplicateFormatChunk => write!(formatter, "WAV contains more than one fmt chunk"),
            Self::UnsupportedFormatTag(tag) => write!(formatter, "WAV format tag 0x{tag:04X} is not integer PCM"),
            Self::InvalidExtensibleFormat(message) => write!(formatter, "invalid WAVE_FORMAT_EXTENSIBLE: {message}"),
            Self::UnsupportedChannels { channels, limit } => write!(formatter, "WAV channel count {channels} exceeds limit {limit}"),
            Self::UnsupportedBitsPerSample(bits) => write!(formatter, "unsupported integer PCM sample width {bits}"),
            Self::InvalidSampleRate(rate) => write!(formatter, "invalid WAV sample rate {rate}"),
            Self::InvalidBlockAlign { declared, expected } => write!(formatter, "WAV block align {declared} differs from expected {expected}"),
            Self::InvalidByteRate { declared, expected } => write!(formatter, "WAV byte rate {declared} differs from expected {expected}"),
            Self::PartialSampleFrame { data_bytes, block_align } => write!(formatter, "{data_bytes} audio bytes are not divisible by block align {block_align}"),
            Self::MetadataLimitExceeded { bytes, limit } => write!(formatter, "mapped metadata requires {bytes} bytes, exceeding limit {limit}"),
            Self::DeclaredRiffSizeExceedsInput { declared_end, actual } => write!(formatter, "RIFF declares an end at {declared_end}, beyond {actual} input bytes"),
            Self::AiffSizeLimitExceeded { bytes } => write!(formatter, "AIFF would require {bytes} bytes, exceeding classic 32-bit FORM limits"),
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

/// Converts integer PCM audio in a RIFF/WAVE file into classic AIFF.
///
/// The reader is seekable because RIFF permits metadata and `fmt ` chunks to
/// appear after audio chunks. Output is streamed and does not require seeking.
pub fn convert<R: Read + Seek + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.buffer_size == 0 {
        return Err(Error::InvalidOptions("buffer_size must be non-zero"));
    }
    if options.buffer_size > MAX_BUFFER_SIZE {
        return Err(Error::InvalidOptions("buffer_size exceeds 16 MiB"));
    }
    if options.max_channels == 0 {
        return Err(Error::InvalidOptions("max_channels must be non-zero"));
    }

    let parsed = wave::inspect(input, options)?;
    let written = aiff::write(input, output, &parsed, options)?;
    Ok(Report {
        input_bytes: parsed.file_size,
        output_bytes: written.output_bytes,
        channels: parsed.format.channels,
        sample_rate: parsed.format.sample_rate,
        container_bits_per_sample: parsed.format.bits_per_sample,
        valid_bits_per_sample: parsed.format.valid_bits_per_sample,
        sample_frames: parsed.sample_frames,
        source_audio_bytes: parsed.audio_bytes,
        output_audio_bytes: written.output_audio_bytes,
        source_data_chunks: parsed.data_segments.len() as u32,
        wave_format_extensible: parsed.format.extensible,
        metadata_chunks_found: parsed.metadata_found,
        metadata_chunks_preserved: parsed.metadata.len() as u32,
        metadata_bytes_preserved: parsed.metadata.iter().map(|item| item.data.len() as u64).sum(),
        peak_working_memory_bytes: (options.buffer_size as u64)
            .checked_mul(2)
            .and_then(|value| {
                value.checked_add(
                    parsed.metadata.iter().map(|item| item.data.len() as u64).sum::<u64>(),
                )
            })
            .ok_or(Error::IntegerOverflow("working memory estimate"))?,
        strategy: "pcm-exact",
        backend: "native-portable",
        warnings: parsed.warnings,
    })
}
