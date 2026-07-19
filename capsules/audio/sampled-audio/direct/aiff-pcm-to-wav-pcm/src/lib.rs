#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

const PCM_GUID: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataPolicy { CommonText, Discard }

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
        Self { metadata: MetadataPolicy::CommonText, strict_header_consistency: true, buffer_size: 65_536, max_metadata_bytes: 1_048_576, max_channels: 256 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub source_audio_bytes: u64,
    pub output_audio_bytes: u64,
    pub channels: u16,
    pub sample_rate: u32,
    pub container_bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    pub sample_frames: u64,
    pub source_sound_chunks: usize,
    pub wave_format_extensible: bool,
    pub metadata_chunks_found: u32,
    pub metadata_chunks_preserved: u32,
    pub peak_working_memory_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error), InvalidOptions(&'static str), InvalidAiff(&'static str), Unsupported(&'static str),
    MetadataLimitExceeded { bytes: u64, limit: u64 }, ContainerTooLarge, ArithmeticOverflow,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
        Self::Io(e) => e.fmt(f), Self::InvalidOptions(m) | Self::InvalidAiff(m) | Self::Unsupported(m) => f.write_str(m),
        Self::MetadataLimitExceeded { bytes, limit } => write!(f, "mapped metadata size {bytes} exceeds limit {limit}"),
        Self::ContainerTooLarge => f.write_str("output exceeds classic RIFF/WAVE size limits"), Self::ArithmeticOverflow => f.write_str("size arithmetic overflow"),
    }}
}
impl std::error::Error for Error { fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { match self { Self::Io(e) => Some(e), _ => None } } }
impl From<io::Error> for Error { fn from(value: io::Error) -> Self { Self::Io(value) } }

#[derive(Clone)] struct Sound { offset: u64, size: u64 }
#[derive(Clone)] struct Metadata { id: [u8; 4], data: Vec<u8> }
struct Parsed { input_bytes: u64, channels: u16, declared_frames: u32, sample_bits: u16, sample_rate: u32, sounds: Vec<Sound>, audio_bytes: u64, metadata: Vec<Metadata>, metadata_found: u32, warnings: Vec<String> }

fn be_u16(b: &[u8]) -> u16 { u16::from_be_bytes([b[0], b[1]]) }
fn be_u32(b: &[u8]) -> u32 { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) }

fn decode_extended_integer(bytes: &[u8]) -> Result<u32, Error> {
    if bytes.len() != 10 { return Err(Error::InvalidAiff("sample rate is not an 80-bit extended value")); }
    let raw_exponent = be_u16(&bytes[0..2]);
    if raw_exponent & 0x8000 != 0 { return Err(Error::Unsupported("negative AIFF sample rate")); }
    let exponent = raw_exponent & 0x7fff;
    if exponent == 0 || exponent == 0x7fff { return Err(Error::Unsupported("non-finite, denormal or zero AIFF sample rate")); }
    let mantissa = u64::from_be_bytes(bytes[2..10].try_into().expect("ten-byte rate has eight-byte mantissa"));
    if mantissa & (1_u64 << 63) == 0 { return Err(Error::InvalidAiff("AIFF extended sample rate has no integer bit")); }
    let shift = i32::from(exponent) - 16_383 - 63;
    let value = if shift >= 0 {
        mantissa.checked_shl(shift as u32).ok_or(Error::Unsupported("AIFF sample rate is too large"))?
    } else {
        let right = (-shift) as u32;
        if right >= 64 { return Err(Error::Unsupported("AIFF sample rate is fractional or too small")); }
        let mask = if right == 0 { 0 } else { (1_u64 << right) - 1 };
        if mantissa & mask != 0 { return Err(Error::Unsupported("fractional AIFF sample rates are not supported")); }
        mantissa >> right
    };
    u32::try_from(value).ok().filter(|v| *v != 0).ok_or(Error::Unsupported("AIFF sample rate is outside the WAVE integer range"))
}

fn mapped_info_id(id: &[u8; 4]) -> Option<[u8; 4]> { match id { b"NAME" => Some(*b"INAM"), b"AUTH" => Some(*b"IART"), b"ANNO" => Some(*b"ICMT"), b"(c) " => Some(*b"ICOP"), _ => None } }

fn inspect<R: Read + Seek>(input: &mut R, options: &Options) -> Result<Parsed, Error> {
    let start = input.stream_position()?;
    let physical_end = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(start))?;
    let mut header = [0_u8; 12]; input.read_exact(&mut header)?;
    if &header[0..4] != b"FORM" { return Err(Error::InvalidAiff("input is not an IFF FORM")); }
    if &header[8..12] == b"AIFC" { return Err(Error::Unsupported("AIFC input is not supported by this classic AIFF Capsule")); }
    if &header[8..12] != b"AIFF" { return Err(Error::InvalidAiff("FORM type is not AIFF")); }
    let form_end = start.checked_add(8).and_then(|v| v.checked_add(u64::from(be_u32(&header[4..8])))).ok_or(Error::ArithmeticOverflow)?;
    if form_end > physical_end { return Err(Error::InvalidAiff("FORM size extends past EOF")); }
    if options.strict_header_consistency && form_end != physical_end { return Err(Error::InvalidAiff("FORM size does not match physical input length")); }
    let mut common: Option<(u16, u32, u16, u32)> = None;
    let mut sounds = Vec::new(); let mut metadata = Vec::new(); let mut metadata_found = 0_u32; let mut metadata_bytes = 0_u64; let mut warnings = Vec::new(); let mut unknown = 0_u32;
    while input.stream_position()? < form_end {
        let position = input.stream_position()?;
        if form_end - position < 8 { return Err(Error::InvalidAiff("truncated AIFF chunk header")); }
        let mut chunk = [0_u8; 8]; input.read_exact(&mut chunk)?;
        let id: [u8; 4] = chunk[0..4].try_into().expect("four-byte id"); let size = u64::from(be_u32(&chunk[4..8])); let payload = input.stream_position()?;
        let payload_end = payload.checked_add(size).ok_or(Error::ArithmeticOverflow)?; let next = payload_end.checked_add(size & 1).ok_or(Error::ArithmeticOverflow)?;
        if next > form_end { return Err(Error::InvalidAiff("AIFF chunk extends past FORM boundary")); }
        match &id {
            b"COMM" => {
                if common.is_some() { return Err(Error::InvalidAiff("duplicate COMM chunk")); }
                if size < 18 { return Err(Error::InvalidAiff("COMM chunk is shorter than 18 bytes")); }
                let mut bytes = [0_u8; 18]; input.read_exact(&mut bytes)?;
                common = Some((be_u16(&bytes[0..2]), be_u32(&bytes[2..6]), be_u16(&bytes[6..8]), decode_extended_integer(&bytes[8..18])?));
            }
            b"SSND" => {
                if size < 8 { return Err(Error::InvalidAiff("SSND chunk is shorter than offset and block size")); }
                let mut fields = [0_u8; 8]; input.read_exact(&mut fields)?;
                let offset = u64::from(be_u32(&fields[0..4])); let block_size = be_u32(&fields[4..8]);
                if offset > size - 8 { return Err(Error::InvalidAiff("SSND offset exceeds chunk payload")); }
                if block_size != 0 { warnings.push(format!("SSND blockSize {block_size} is informational and was normalized")); }
                sounds.push(Sound { offset: payload + 8 + offset, size: size - 8 - offset });
            }
            b"NAME" | b"AUTH" | b"ANNO" | b"(c) " => {
                metadata_found = metadata_found.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                if options.metadata == MetadataPolicy::CommonText {
                    metadata_bytes = metadata_bytes.checked_add(size).ok_or(Error::ArithmeticOverflow)?;
                    if metadata_bytes > options.max_metadata_bytes { return Err(Error::MetadataLimitExceeded { bytes: metadata_bytes, limit: options.max_metadata_bytes }); }
                    let mut data = vec![0_u8; size as usize]; input.read_exact(&mut data)?; while data.last() == Some(&0) { data.pop(); }
                    metadata.push(Metadata { id: mapped_info_id(&id).expect("matched metadata id"), data });
                }
            }
            _ => unknown += 1,
        }
        input.seek(SeekFrom::Start(next))?;
    }
    let (channels, declared_frames, sample_bits, sample_rate) = common.ok_or(Error::InvalidAiff("missing COMM chunk"))?;
    if channels == 0 || channels > options.max_channels { return Err(Error::Unsupported("AIFF channel count is unsupported")); }
    if !(1..=32).contains(&sample_bits) { return Err(Error::Unsupported("AIFF sample size must be between 1 and 32 bits")); }
    if sounds.is_empty() { return Err(Error::InvalidAiff("missing SSND chunk")); }
    let audio_bytes = sounds.iter().try_fold(0_u64, |sum, sound| sum.checked_add(sound.size).ok_or(Error::ArithmeticOverflow))?;
    if unknown != 0 { warnings.push(format!("{unknown} unrecognized AIFF chunks were not mapped to WAVE")); }
    Ok(Parsed { input_bytes: physical_end - start, channels, declared_frames, sample_bits, sample_rate, sounds, audio_bytes, metadata, metadata_found, warnings })
}

fn push_info(list: &mut Vec<u8>, item: &Metadata) -> Result<(), Error> {
    list.extend_from_slice(&item.id);
    let size = item.data.len().checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    list.extend_from_slice(&u32::try_from(size).map_err(|_| Error::ContainerTooLarge)?.to_le_bytes());
    list.extend_from_slice(&item.data); list.push(0); if size & 1 != 0 { list.push(0); } Ok(())
}

/// Convert classic AIFF integer PCM to RIFF/WAVE integer PCM.
pub fn convert<R: Read + Seek, W: Write>(input: &mut R, output: &mut W, options: &Options) -> Result<Report, Error> {
    if !(256..=16 * 1024 * 1024).contains(&options.buffer_size) { return Err(Error::InvalidOptions("buffer_size must be between 256 bytes and 16 MiB")); }
    let parsed = inspect(input, options)?;
    let sample_bytes = usize::from(parsed.sample_bits.div_ceil(8)); let container_bits = (sample_bytes * 8) as u16;
    let block_align = usize::from(parsed.channels).checked_mul(sample_bytes).ok_or(Error::ArithmeticOverflow)?;
    for sound in &parsed.sounds { if sound.size % block_align as u64 != 0 { return Err(Error::InvalidAiff("SSND audio is not aligned to complete PCM frames")); } }
    let actual_remainder = parsed.audio_bytes % block_align as u64;
    let actual_frames = parsed.audio_bytes / block_align as u64;
    if options.strict_header_consistency && (actual_remainder != 0 || actual_frames != u64::from(parsed.declared_frames)) { return Err(Error::InvalidAiff("COMM frame count does not match complete SSND audio frames")); }
    let output_audio = actual_frames.checked_mul(block_align as u64).ok_or(Error::ArithmeticOverflow)?;
    let data_size = u32::try_from(output_audio).map_err(|_| Error::ContainerTooLarge)?;
    let extensible = parsed.sample_bits != container_bits;
    let fmt_size = if extensible { 40_u32 } else { 16_u32 };
    let mut list = Vec::new(); if !parsed.metadata.is_empty() { list.extend_from_slice(b"INFO"); for item in &parsed.metadata { push_info(&mut list, item)?; } }
    let list_total = if list.is_empty() { 0_u64 } else { 8 + list.len() as u64 + (list.len() as u64 & 1) };
    let data_total = 8_u64 + output_audio + (output_audio & 1);
    let riff_size = 4_u64.checked_add(8 + u64::from(fmt_size)).and_then(|v| v.checked_add(list_total)).and_then(|v| v.checked_add(data_total)).ok_or(Error::ArithmeticOverflow)?;
    let riff_u32 = u32::try_from(riff_size).map_err(|_| Error::ContainerTooLarge)?;
    let byte_rate = parsed.sample_rate.checked_mul(block_align as u32).ok_or(Error::ArithmeticOverflow)?;
    output.write_all(b"RIFF")?; output.write_all(&riff_u32.to_le_bytes())?; output.write_all(b"WAVEfmt ")?; output.write_all(&fmt_size.to_le_bytes())?;
    output.write_all(&(if extensible { 0xfffe_u16 } else { 1_u16 }).to_le_bytes())?; output.write_all(&parsed.channels.to_le_bytes())?; output.write_all(&parsed.sample_rate.to_le_bytes())?; output.write_all(&byte_rate.to_le_bytes())?; output.write_all(&(block_align as u16).to_le_bytes())?; output.write_all(&container_bits.to_le_bytes())?;
    if extensible { output.write_all(&22_u16.to_le_bytes())?; output.write_all(&parsed.sample_bits.to_le_bytes())?; output.write_all(&0_u32.to_le_bytes())?; output.write_all(&PCM_GUID)?; }
    if !list.is_empty() { output.write_all(b"LIST")?; output.write_all(&(list.len() as u32).to_le_bytes())?; output.write_all(&list)?; if list.len() & 1 != 0 { output.write_all(&[0])?; } }
    output.write_all(b"data")?; output.write_all(&data_size.to_le_bytes())?;
    let mut buffer_len = options.buffer_size - (options.buffer_size % block_align); if buffer_len == 0 { buffer_len = block_align; }
    let mut buffer = vec![0_u8; buffer_len]; let mut remaining_total = output_audio;
    for sound in &parsed.sounds { if remaining_total == 0 { break; } input.seek(SeekFrom::Start(sound.offset))?; let mut remaining = sound.size.min(remaining_total); while remaining != 0 {
        let count = remaining.min(buffer.len() as u64) as usize; input.read_exact(&mut buffer[..count])?;
        for sample in buffer[..count].chunks_exact_mut(sample_bytes) { if sample_bytes == 1 { sample[0] ^= 0x80; } else { sample.reverse(); } }
        output.write_all(&buffer[..count])?; remaining -= count as u64; remaining_total -= count as u64;
    }}
    if output_audio & 1 != 0 { output.write_all(&[0])?; }
    let mut warnings = parsed.warnings; if actual_remainder != 0 { warnings.push(format!("discarded {actual_remainder} trailing SSND byte(s)")); } if actual_frames != u64::from(parsed.declared_frames) { warnings.push(format!("COMM declared {} frames; emitted {actual_frames}", parsed.declared_frames)); }
    Ok(Report { input_bytes: parsed.input_bytes, output_bytes: riff_size + 8, source_audio_bytes: parsed.audio_bytes, output_audio_bytes: output_audio, channels: parsed.channels, sample_rate: parsed.sample_rate, container_bits_per_sample: container_bits, valid_bits_per_sample: parsed.sample_bits, sample_frames: actual_frames, source_sound_chunks: parsed.sounds.len(), wave_format_extensible: extensible, metadata_chunks_found: parsed.metadata_found, metadata_chunks_preserved: parsed.metadata.len() as u32, peak_working_memory_bytes: buffer.len() as u64 + list.len() as u64 + options.max_metadata_bytes.min(parsed.metadata.iter().map(|m| m.data.len() as u64).sum()), warnings })
}
