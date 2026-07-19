use std::io::{Read, Seek, SeekFrom};

use crate::{Error, MetadataPolicy, Options};

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const PCM_SUBFORMAT_GUID: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

#[derive(Debug, Clone)]
pub(crate) struct Format {
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) block_align: u16,
    pub(crate) bits_per_sample: u16,
    pub(crate) valid_bits_per_sample: u16,
    pub(crate) extensible: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct DataSegment {
    pub(crate) offset: u64,
    pub(crate) size: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataChunk {
    pub(crate) id: [u8; 4],
    pub(crate) data: Vec<u8>,
}

pub(crate) struct ParsedWave {
    pub(crate) file_size: u64,
    pub(crate) format: Format,
    pub(crate) data_segments: Vec<DataSegment>,
    pub(crate) audio_bytes: u64,
    pub(crate) sample_frames: u32,
    pub(crate) metadata: Vec<MetadataChunk>,
    pub(crate) metadata_found: u32,
    pub(crate) warnings: Vec<String>,
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn read_exact<R: Read + ?Sized>(
    input: &mut R,
    buffer: &mut [u8],
    region: &'static str,
) -> Result<(), Error> {
    input.read_exact(buffer).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Truncated(region)
        } else {
            Error::Io(error)
        }
    })
}

fn parse_format<R: Read + Seek + ?Sized>(
    input: &mut R,
    offset: u64,
    size: u32,
    options: &Options,
    warnings: &mut Vec<String>,
) -> Result<Format, Error> {
    if size < 16 {
        return Err(Error::Truncated("fmt chunk"));
    }
    input.seek(SeekFrom::Start(offset))?;
    let read_size = (size as usize).min(40);
    let mut bytes = vec![0_u8; read_size];
    read_exact(input, &mut bytes, "fmt chunk")?;

    let format_tag = le_u16(&bytes[0..2]);
    let channels = le_u16(&bytes[2..4]);
    let sample_rate = le_u32(&bytes[4..8]);
    let byte_rate = le_u32(&bytes[8..12]);
    let block_align = le_u16(&bytes[12..14]);
    let bits_per_sample = le_u16(&bytes[14..16]);
    if channels == 0 || channels > options.max_channels {
        return Err(Error::UnsupportedChannels {
            channels,
            limit: options.max_channels,
        });
    }
    if sample_rate == 0 {
        return Err(Error::InvalidSampleRate(sample_rate));
    }
    if !matches!(bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(Error::UnsupportedBitsPerSample(bits_per_sample));
    }

    let (valid_bits_per_sample, extensible) = match format_tag {
        WAVE_FORMAT_PCM => (bits_per_sample, false),
        WAVE_FORMAT_EXTENSIBLE => {
            if size < 40 || bytes.len() < 40 {
                return Err(Error::InvalidExtensibleFormat("fmt chunk is shorter than 40 bytes"));
            }
            let extension_size = le_u16(&bytes[16..18]);
            if extension_size < 22 {
                return Err(Error::InvalidExtensibleFormat("extension size is shorter than 22 bytes"));
            }
            if 18_u32 + extension_size as u32 > size {
                return Err(Error::InvalidExtensibleFormat("extension size exceeds the fmt chunk"));
            }
            let valid_bits = le_u16(&bytes[18..20]);
            if valid_bits == 0 || valid_bits > bits_per_sample {
                return Err(Error::InvalidExtensibleFormat("valid bits are zero or exceed the container width"));
            }
            if bytes[24..40] != PCM_SUBFORMAT_GUID {
                return Err(Error::UnsupportedFormatTag(format_tag));
            }
            (valid_bits, true)
        }
        other => return Err(Error::UnsupportedFormatTag(other)),
    };

    let sample_bytes = bits_per_sample / 8;
    let expected_block_align = channels
        .checked_mul(sample_bytes)
        .ok_or(Error::IntegerOverflow("WAV block align"))?;
    if block_align != expected_block_align {
        if options.strict_header_consistency {
            return Err(Error::InvalidBlockAlign {
                declared: block_align,
                expected: expected_block_align,
            });
        }
        warnings.push(format!(
            "declared block align {block_align} was normalized to {expected_block_align}"
        ));
    }
    let expected_byte_rate = sample_rate
        .checked_mul(expected_block_align as u32)
        .ok_or(Error::IntegerOverflow("WAV byte rate"))?;
    if byte_rate != expected_byte_rate {
        if options.strict_header_consistency {
            return Err(Error::InvalidByteRate {
                declared: byte_rate,
                expected: expected_byte_rate,
            });
        }
        warnings.push(format!(
            "declared byte rate {byte_rate} was normalized to {expected_byte_rate}"
        ));
    }

    Ok(Format {
        channels,
        sample_rate,
        block_align: expected_block_align,
        bits_per_sample,
        valid_bits_per_sample,
        extensible,
    })
}

fn mapped_info_id(id: &[u8; 4]) -> Option<[u8; 4]> {
    match id {
        b"INAM" => Some(*b"NAME"),
        b"IART" => Some(*b"AUTH"),
        b"ICMT" => Some(*b"ANNO"),
        b"ICOP" => Some(*b"(c) "),
        _ => None,
    }
}

fn parse_info_list<R: Read + Seek + ?Sized>(
    input: &mut R,
    offset: u64,
    size: u32,
    options: &Options,
    metadata: &mut Vec<MetadataChunk>,
    metadata_found: &mut u32,
    metadata_bytes: &mut u64,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    if size < 4 {
        warnings.push("ignored LIST chunk shorter than its four-byte list type".into());
        return Ok(());
    }
    input.seek(SeekFrom::Start(offset))?;
    let mut list_type = [0_u8; 4];
    read_exact(input, &mut list_type, "LIST type")?;
    if &list_type != b"INFO" {
        return Ok(());
    }
    let list_end = offset
        .checked_add(size as u64)
        .ok_or(Error::IntegerOverflow("LIST end"))?;
    let mut position = offset + 4;
    while position + 8 <= list_end {
        input.seek(SeekFrom::Start(position))?;
        let mut header = [0_u8; 8];
        read_exact(input, &mut header, "INFO subchunk header")?;
        let id: [u8; 4] = header[0..4].try_into().expect("four-byte slice");
        let data_size = le_u32(&header[4..8]) as u64;
        let data_offset = position + 8;
        let data_end = data_offset
            .checked_add(data_size)
            .ok_or(Error::IntegerOverflow("INFO subchunk end"))?;
        if data_end > list_end {
            return Err(Error::Truncated("INFO subchunk"));
        }
        if let Some(output_id) = mapped_info_id(&id) {
            *metadata_found = metadata_found
                .checked_add(1)
                .ok_or(Error::IntegerOverflow("metadata chunk count"))?;
            if options.metadata == MetadataPolicy::CommonText {
                let allocation = usize::try_from(data_size)
                    .map_err(|_| Error::IntegerOverflow("metadata allocation"))?;
                let mut data = vec![0_u8; allocation];
                input.seek(SeekFrom::Start(data_offset))?;
                read_exact(input, &mut data, "INFO metadata")?;
                while data.last() == Some(&0) {
                    data.pop();
                }
                let next_total = metadata_bytes
                    .checked_add(data.len() as u64)
                    .ok_or(Error::IntegerOverflow("metadata byte count"))?;
                if next_total > options.max_metadata_bytes {
                    return Err(Error::MetadataLimitExceeded {
                        bytes: next_total,
                        limit: options.max_metadata_bytes,
                    });
                }
                *metadata_bytes = next_total;
                metadata.push(MetadataChunk { id: output_id, data });
            }
        }
        position = data_end
            .checked_add(data_size & 1)
            .ok_or(Error::IntegerOverflow("INFO padding"))?;
    }
    if position != list_end {
        warnings.push("LIST/INFO contains trailing bytes that do not form a complete subchunk".into());
    }
    Ok(())
}

pub(crate) fn inspect<R: Read + Seek + ?Sized>(
    input: &mut R,
    options: &Options,
) -> Result<ParsedWave, Error> {
    let file_size = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;
    let mut header = [0_u8; 12];
    read_exact(input, &mut header, "RIFF header")?;
    let signature: [u8; 4] = header[0..4].try_into().expect("four-byte slice");
    match &signature {
        b"RIFF" => {}
        b"RF64" | b"BW64" => return Err(Error::UnsupportedRf64),
        b"RIFX" => return Err(Error::UnsupportedRifx),
        _ => return Err(Error::InvalidSignature(signature)),
    }
    if &header[8..12] != b"WAVE" {
        return Err(Error::InvalidWaveForm);
    }
    let riff_size = le_u32(&header[4..8]) as u64;
    if riff_size < 4 {
        return Err(Error::Truncated("RIFF form"));
    }
    let riff_end = 8_u64
        .checked_add(riff_size)
        .ok_or(Error::IntegerOverflow("RIFF end"))?;
    if riff_end > file_size {
        return Err(Error::DeclaredRiffSizeExceedsInput {
            declared_end: riff_end,
            actual: file_size,
        });
    }

    let mut warnings = Vec::new();
    if riff_end != file_size {
        warnings.push(format!(
            "RIFF ends at byte {riff_end}, leaving {} trailing input bytes",
            file_size - riff_end
        ));
    }
    let mut format = None;
    let mut data_segments = Vec::new();
    let mut metadata = Vec::new();
    let mut metadata_found = 0_u32;
    let mut metadata_bytes = 0_u64;
    let mut unknown_chunks = 0_u32;
    let mut position = 12_u64;
    while position + 8 <= riff_end {
        input.seek(SeekFrom::Start(position))?;
        let mut chunk_header = [0_u8; 8];
        read_exact(input, &mut chunk_header, "chunk header")?;
        let id: [u8; 4] = chunk_header[0..4].try_into().expect("four-byte slice");
        let size = le_u32(&chunk_header[4..8]);
        let data_offset = position + 8;
        let data_end = data_offset
            .checked_add(size as u64)
            .ok_or(Error::IntegerOverflow("chunk end"))?;
        let next = data_end
            .checked_add((size as u64) & 1)
            .ok_or(Error::IntegerOverflow("chunk padding"))?;
        if next > riff_end {
            return Err(Error::Truncated("chunk payload or padding"));
        }
        match &id {
            b"fmt " => {
                if format.is_some() {
                    return Err(Error::DuplicateFormatChunk);
                }
                format = Some(parse_format(input, data_offset, size, options, &mut warnings)?);
            }
            b"data" => data_segments.push(DataSegment {
                offset: data_offset,
                size: size as u64,
            }),
            b"LIST" => parse_info_list(
                input,
                data_offset,
                size,
                options,
                &mut metadata,
                &mut metadata_found,
                &mut metadata_bytes,
                &mut warnings,
            )?,
            _ => unknown_chunks += 1,
        }
        position = next;
    }
    if position != riff_end {
        warnings.push("RIFF contains trailing bytes that do not form a complete chunk header".into());
    }
    if unknown_chunks != 0 {
        warnings.push(format!("{unknown_chunks} unrecognized WAV chunks were not mapped to AIFF"));
    }
    let format = format.ok_or(Error::MissingFormatChunk)?;
    if data_segments.is_empty() {
        return Err(Error::MissingDataChunk);
    }
    let mut audio_bytes = 0_u64;
    for segment in &data_segments {
        if segment.size % format.block_align as u64 != 0 {
            return Err(Error::PartialSampleFrame {
                data_bytes: segment.size,
                block_align: format.block_align,
            });
        }
        audio_bytes = audio_bytes
            .checked_add(segment.size)
            .ok_or(Error::IntegerOverflow("audio byte count"))?;
    }
    let frames = audio_bytes / format.block_align as u64;
    let sample_frames = u32::try_from(frames)
        .map_err(|_| Error::AiffSizeLimitExceeded { bytes: audio_bytes })?;

    Ok(ParsedWave {
        file_size,
        format,
        data_segments,
        audio_bytes,
        sample_frames,
        metadata,
        metadata_found,
        warnings,
    })
}
