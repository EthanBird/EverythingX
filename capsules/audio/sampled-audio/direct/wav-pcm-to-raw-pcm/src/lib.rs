#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

const PCM_GUID: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness { Little, Big }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerEncoding { Signed, Unsigned }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub output_endianness: Endianness,
    pub output_encoding: IntegerEncoding,
    pub strict_header_consistency: bool,
    pub buffer_size: usize,
    pub max_channels: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output_endianness: Endianness::Little,
            output_encoding: IntegerEncoding::Signed,
            strict_header_consistency: true,
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
    pub container_bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    pub sample_frames: u64,
    pub source_data_chunks: usize,
    pub wave_format_extensible: bool,
    pub signedness_changed: bool,
    pub byte_order_changed: bool,
    pub peak_working_memory_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidOptions(&'static str),
    InvalidWave(&'static str),
    Unsupported(&'static str),
    MisalignedData { bytes: u64, frame_bytes: u64 },
    ArithmeticOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::InvalidOptions(message) | Self::InvalidWave(message) | Self::Unsupported(message) => f.write_str(message),
            Self::MisalignedData { bytes, frame_bytes } => write!(f, "WAVE data length {bytes} is not aligned to {frame_bytes}-byte frames"),
            Self::ArithmeticOverflow => f.write_str("size arithmetic overflow"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io(error) => Some(error), _ => None }
    }
}
impl From<io::Error> for Error { fn from(value: io::Error) -> Self { Self::Io(value) } }

#[derive(Debug, Clone)]
struct Format {
    channels: u16,
    sample_rate: u32,
    bits: u16,
    valid_bits: u16,
    block_align: u16,
    extensible: bool,
}

fn read_u16(bytes: &[u8]) -> u16 { u16::from_le_bytes([bytes[0], bytes[1]]) }
fn read_u32(bytes: &[u8]) -> u32 { u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }

fn parse_format(bytes: &[u8], options: &Options) -> Result<Format, Error> {
    if bytes.len() < 16 { return Err(Error::InvalidWave("fmt chunk is shorter than 16 bytes")); }
    let tag = read_u16(&bytes[0..2]);
    let channels = read_u16(&bytes[2..4]);
    let sample_rate = read_u32(&bytes[4..8]);
    let byte_rate = read_u32(&bytes[8..12]);
    let block_align = read_u16(&bytes[12..14]);
    let bits = read_u16(&bytes[14..16]);
    if channels == 0 || channels > options.max_channels { return Err(Error::Unsupported("channel count is unsupported")); }
    if sample_rate == 0 { return Err(Error::InvalidWave("sample rate is zero")); }
    if !matches!(bits, 8 | 16 | 24 | 32) { return Err(Error::Unsupported("only 8/16/24/32-bit integer PCM is supported")); }
    let (valid_bits, extensible) = match tag {
        1 => (bits, false),
        0xfffe => {
            if bytes.len() < 40 || read_u16(&bytes[16..18]) < 22 { return Err(Error::InvalidWave("extensible fmt chunk is incomplete")); }
            if bytes[24..40] != PCM_GUID { return Err(Error::Unsupported("WAVE extensible subtype is not integer PCM")); }
            let valid = read_u16(&bytes[18..20]);
            if valid == 0 || valid > bits { return Err(Error::InvalidWave("valid bits per sample is invalid")); }
            (valid, true)
        }
        _ => return Err(Error::Unsupported("WAVE format is not integer PCM")),
    };
    let expected_align = u32::from(channels) * u32::from(bits / 8);
    let expected_rate = sample_rate.checked_mul(expected_align).ok_or(Error::ArithmeticOverflow)?;
    if u32::from(block_align) != expected_align { return Err(Error::InvalidWave("block alignment does not match PCM format")); }
    if options.strict_header_consistency && byte_rate != expected_rate { return Err(Error::InvalidWave("byte rate does not match PCM format")); }
    Ok(Format { channels, sample_rate, bits, valid_bits, block_align, extensible })
}

/// Extract integer PCM from RIFF/WAVE and emit parameterized headerless PCM.
/// Default output is signed, little-endian interleaved PCM. The returned report
/// carries every parameter needed to interpret the raw output.
pub fn convert<R: Read + Seek, W: Write>(input: &mut R, output: &mut W, options: &Options) -> Result<Report, Error> {
    if !(256..=16 * 1024 * 1024).contains(&options.buffer_size) { return Err(Error::InvalidOptions("buffer_size must be between 256 bytes and 16 MiB")); }
    let start = input.stream_position()?;
    let physical_end = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(start))?;
    let mut header = [0_u8; 12];
    input.read_exact(&mut header)?;
    if &header[0..4] == b"RIFX" { return Err(Error::Unsupported("big-endian RIFX input is not supported")); }
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" { return Err(Error::InvalidWave("input is not RIFF/WAVE")); }
    let declared_end = start.checked_add(8).and_then(|v| v.checked_add(u64::from(read_u32(&header[4..8])))).ok_or(Error::ArithmeticOverflow)?;
    if declared_end > physical_end { return Err(Error::InvalidWave("RIFF size extends past EOF")); }
    if options.strict_header_consistency && declared_end != physical_end { return Err(Error::InvalidWave("RIFF size does not match physical input length")); }
    let scan_end = declared_end.min(physical_end);
    let mut format: Option<Format> = None;
    let mut data_chunks = Vec::<(u64, u64)>::new();
    while input.stream_position()? < scan_end {
        let position = input.stream_position()?;
        if scan_end - position < 8 { return Err(Error::InvalidWave("truncated chunk header")); }
        let mut chunk_header = [0_u8; 8];
        input.read_exact(&mut chunk_header)?;
        let size = u64::from(read_u32(&chunk_header[4..8]));
        let payload = input.stream_position()?;
        let payload_end = payload.checked_add(size).ok_or(Error::ArithmeticOverflow)?;
        let next = payload_end.checked_add(size & 1).ok_or(Error::ArithmeticOverflow)?;
        if payload_end > scan_end || next > physical_end { return Err(Error::InvalidWave("chunk extends past RIFF boundary")); }
        let chunk_id = &chunk_header[0..4];
        if chunk_id == b"fmt " {
                if size > 4096 { return Err(Error::Unsupported("fmt chunk is unreasonably large")); }
                let mut bytes = vec![0_u8; size as usize];
                input.read_exact(&mut bytes)?;
                let candidate = parse_format(&bytes, options)?;
                if let Some(existing) = &format {
                    if existing.channels != candidate.channels || existing.sample_rate != candidate.sample_rate || existing.bits != candidate.bits || existing.valid_bits != candidate.valid_bits {
                        return Err(Error::InvalidWave("conflicting fmt chunks"));
                    }
                } else { format = Some(candidate); }
        } else if chunk_id == b"data" {
            data_chunks.push((payload, size));
        }
        input.seek(SeekFrom::Start(next))?;
    }
    let format = format.ok_or(Error::InvalidWave("missing fmt chunk"))?;
    if data_chunks.is_empty() { return Err(Error::InvalidWave("missing data chunk")); }
    for (_, size) in &data_chunks {
        if size % u64::from(format.block_align) != 0 {
            return Err(Error::MisalignedData { bytes: *size, frame_bytes: u64::from(format.block_align) });
        }
    }
    let total_data = data_chunks.iter().try_fold(0_u64, |sum, (_, size)| sum.checked_add(*size).ok_or(Error::ArithmeticOverflow))?;
    let remainder = 0;
    let output_audio = total_data;
    let sample_bytes = usize::from(format.bits / 8);
    let source_signed = sample_bytes != 1;
    let target_signed = options.output_encoding == IntegerEncoding::Signed;
    let mut buffer_len = options.buffer_size - (options.buffer_size % usize::from(format.block_align));
    if buffer_len == 0 { buffer_len = usize::from(format.block_align); }
    let mut buffer = vec![0_u8; buffer_len];
    let mut remaining_total = output_audio;
    for (offset, size) in &data_chunks {
        if remaining_total == 0 { break; }
        input.seek(SeekFrom::Start(*offset))?;
        let mut remaining_chunk = (*size).min(remaining_total);
        while remaining_chunk != 0 {
            let count = remaining_chunk.min(buffer.len() as u64) as usize;
            input.read_exact(&mut buffer[..count])?;
            for sample in buffer[..count].chunks_exact_mut(sample_bytes) {
                if source_signed != target_signed { sample[sample_bytes - 1] ^= 0x80; }
                if options.output_endianness == Endianness::Big && sample_bytes > 1 { sample.reverse(); }
            }
            output.write_all(&buffer[..count])?;
            remaining_chunk -= count as u64;
            remaining_total -= count as u64;
        }
    }
    let warnings = if remainder == 0 { Vec::new() } else { vec![format!("discarded {remainder} trailing byte(s) that do not form a complete frame")] };
    Ok(Report {
        input_bytes: physical_end - start,
        output_bytes: output_audio,
        source_audio_bytes: total_data,
        output_audio_bytes: output_audio,
        discarded_trailing_bytes: remainder,
        channels: format.channels,
        sample_rate: format.sample_rate,
        container_bits_per_sample: format.bits,
        valid_bits_per_sample: format.valid_bits,
        sample_frames: output_audio / u64::from(format.block_align),
        source_data_chunks: data_chunks.len(),
        wave_format_extensible: format.extensible,
        signedness_changed: source_signed != target_signed,
        byte_order_changed: options.output_endianness == Endianness::Big && sample_bytes > 1,
        peak_working_memory_bytes: buffer.len() as u64 + 4096,
        warnings,
    })
}
