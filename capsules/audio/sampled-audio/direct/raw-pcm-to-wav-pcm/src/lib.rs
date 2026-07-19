#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

const MIN_BUFFER_SIZE: usize = 256;
const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerEncoding {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub input_endianness: Endianness,
    pub input_encoding: IntegerEncoding,
    pub strict_frame_alignment: bool,
    pub buffer_size: usize,
    pub max_channels: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 16,
            input_endianness: Endianness::Little,
            input_encoding: IntegerEncoding::Signed,
            strict_frame_alignment: true,
            buffer_size: 65_536,
            max_channels: 256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub source_audio_bytes: u64,
    pub output_audio_bytes: u64,
    pub discarded_trailing_bytes: u64,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub sample_frames: u64,
    pub signedness_changed: bool,
    pub byte_order_changed: bool,
    pub peak_working_memory_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidOptions(&'static str),
    MisalignedInput { bytes: u64, frame_bytes: u64 },
    ContainerTooLarge,
    ArithmeticOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidOptions(message) => formatter.write_str(message),
            Self::MisalignedInput { bytes, frame_bytes } => write!(
                formatter,
                "raw PCM length {bytes} is not aligned to {frame_bytes}-byte frames"
            ),
            Self::ContainerTooLarge => formatter.write_str("output exceeds classic RIFF/WAVE size limits"),
            Self::ArithmeticOverflow => formatter.write_str("size arithmetic overflow"),
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

fn validate(options: &Options) -> Result<usize, Error> {
    if options.channels == 0 || options.channels > options.max_channels {
        return Err(Error::InvalidOptions("channels must be between 1 and max_channels"));
    }
    if options.sample_rate == 0 {
        return Err(Error::InvalidOptions("sample_rate must be non-zero"));
    }
    if !matches!(options.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(Error::InvalidOptions("bits_per_sample must be 8, 16, 24 or 32"));
    }
    if !(MIN_BUFFER_SIZE..=MAX_BUFFER_SIZE).contains(&options.buffer_size) {
        return Err(Error::InvalidOptions("buffer_size must be between 256 bytes and 16 MiB"));
    }
    Ok((options.bits_per_sample / 8) as usize)
}

fn write_wave_header<W: Write>(
    output: &mut W,
    options: &Options,
    data_bytes: u32,
) -> Result<(), Error> {
    let sample_bytes = u32::from(options.bits_per_sample / 8);
    let block_align = u32::from(options.channels)
        .checked_mul(sample_bytes)
        .ok_or(Error::ArithmeticOverflow)?;
    let byte_rate = options.sample_rate
        .checked_mul(block_align)
        .ok_or(Error::ArithmeticOverflow)?;
    if block_align > u32::from(u16::MAX) {
        return Err(Error::ArithmeticOverflow);
    }
    let pad = data_bytes & 1;
    let riff_size = 36_u32
        .checked_add(data_bytes)
        .and_then(|value| value.checked_add(pad))
        .ok_or(Error::ContainerTooLarge)?;
    output.write_all(b"RIFF")?;
    output.write_all(&riff_size.to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16_u32.to_le_bytes())?;
    output.write_all(&1_u16.to_le_bytes())?;
    output.write_all(&options.channels.to_le_bytes())?;
    output.write_all(&options.sample_rate.to_le_bytes())?;
    output.write_all(&byte_rate.to_le_bytes())?;
    output.write_all(&(block_align as u16).to_le_bytes())?;
    output.write_all(&options.bits_per_sample.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}

/// Convert raw interleaved integer PCM into canonical little-endian RIFF/WAVE PCM.
///
/// The stream is read from its current position to EOF. Raw PCM is not
/// self-describing, so `Options` owns the required interpretation. `Default`
/// is directly runnable and means mono, 44.1 kHz, signed 16-bit little-endian.
pub fn convert<R: Read + Seek, W: Write>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    let sample_bytes = validate(options)?;
    let frame_bytes = usize::from(options.channels)
        .checked_mul(sample_bytes)
        .ok_or(Error::ArithmeticOverflow)?;
    let start = input.stream_position()?;
    let end = input.seek(SeekFrom::End(0))?;
    if end < start {
        return Err(Error::ArithmeticOverflow);
    }
    input.seek(SeekFrom::Start(start))?;
    let source_bytes = end - start;
    let remainder = source_bytes % frame_bytes as u64;
    if remainder != 0 && options.strict_frame_alignment {
        return Err(Error::MisalignedInput {
            bytes: source_bytes,
            frame_bytes: frame_bytes as u64,
        });
    }
    let audio_bytes = source_bytes - remainder;
    let data_bytes = u32::try_from(audio_bytes).map_err(|_| Error::ContainerTooLarge)?;
    write_wave_header(output, options, data_bytes)?;

    let mut buffer_len = options.buffer_size - (options.buffer_size % frame_bytes);
    if buffer_len == 0 {
        buffer_len = frame_bytes;
    }
    let mut buffer = vec![0_u8; buffer_len];
    let mut remaining = audio_bytes;
    let target_is_signed = sample_bytes != 1;
    let source_is_signed = options.input_encoding == IntegerEncoding::Signed;
    while remaining != 0 {
        let count = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| Error::ArithmeticOverflow)?;
        input.read_exact(&mut buffer[..count])?;
        for sample in buffer[..count].chunks_exact_mut(sample_bytes) {
            if options.input_endianness == Endianness::Big && sample_bytes > 1 {
                sample.reverse();
            }
            if source_is_signed != target_is_signed {
                sample[sample_bytes - 1] ^= 0x80;
            }
        }
        output.write_all(&buffer[..count])?;
        remaining -= count as u64;
    }
    if data_bytes & 1 != 0 {
        output.write_all(&[0])?;
    }
    let output_bytes = 44_u64
        .checked_add(audio_bytes)
        .and_then(|value| value.checked_add(audio_bytes & 1))
        .ok_or(Error::ArithmeticOverflow)?;
    let warnings = if remainder == 0 {
        Vec::new()
    } else {
        vec![format!("discarded {remainder} trailing byte(s) that do not form a complete frame")]
    };
    Ok(Report {
        input_bytes: source_bytes,
        output_bytes,
        source_audio_bytes: source_bytes,
        output_audio_bytes: audio_bytes,
        discarded_trailing_bytes: remainder,
        channels: options.channels,
        sample_rate: options.sample_rate,
        bits_per_sample: options.bits_per_sample,
        sample_frames: audio_bytes / frame_bytes as u64,
        signedness_changed: source_is_signed != target_is_signed,
        byte_order_changed: options.input_endianness == Endianness::Big && sample_bytes > 1,
        peak_working_memory_bytes: buffer.len() as u64,
        warnings,
    })
}
