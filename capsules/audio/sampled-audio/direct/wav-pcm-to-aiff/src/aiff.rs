use std::io::{Read, Seek, SeekFrom, Write};

use crate::wave::{MetadataChunk, ParsedWave};
use crate::{Error, Options};

struct CountingWriter<'a, W: Write + ?Sized> {
    inner: &'a mut W,
    count: u64,
}

impl<W: Write + ?Sized> Write for CountingWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.count += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn chunk_total(data_bytes: u64) -> Result<u64, Error> {
    8_u64
        .checked_add(data_bytes)
        .and_then(|value| value.checked_add(data_bytes & 1))
        .ok_or(Error::IntegerOverflow("AIFF chunk size"))
}

fn write_chunk<W: Write + ?Sized>(
    output: &mut W,
    id: &[u8; 4],
    data: &[u8],
) -> Result<(), Error> {
    let size = u32::try_from(data.len())
        .map_err(|_| Error::AiffSizeLimitExceeded { bytes: data.len() as u64 })?;
    output.write_all(id)?;
    output.write_all(&size.to_be_bytes())?;
    output.write_all(data)?;
    if data.len() & 1 != 0 {
        output.write_all(&[0])?;
    }
    Ok(())
}

fn extended_sample_rate(sample_rate: u32) -> [u8; 10] {
    debug_assert!(sample_rate != 0);
    let power = 31 - sample_rate.leading_zeros();
    let exponent = (16_383 + power) as u16;
    let mantissa = (sample_rate as u64) << (63 - power);
    let mut result = [0_u8; 10];
    result[0..2].copy_from_slice(&exponent.to_be_bytes());
    result[2..10].copy_from_slice(&mantissa.to_be_bytes());
    result
}

fn metadata_total(metadata: &[MetadataChunk]) -> Result<u64, Error> {
    metadata.iter().try_fold(0_u64, |total, item| {
        total
            .checked_add(chunk_total(item.data.len() as u64)?)
            .ok_or(Error::IntegerOverflow("AIFF metadata total"))
    })
}

fn transform_samples(input: &[u8], input_width: usize, output_width: usize, output: &mut Vec<u8>) {
    debug_assert_eq!(input.len() % input_width, 0);
    output.clear();
    output.reserve((input.len() / input_width) * output_width);
    if input_width == 1 {
        for sample in input {
            output.push(*sample ^ 0x80);
        }
        return;
    }
    for sample in input.chunks_exact(input_width) {
        for index in 0..output_width {
            output.push(sample[input_width - 1 - index]);
        }
    }
}

pub(crate) struct WriteStats {
    pub(crate) output_bytes: u64,
    pub(crate) output_audio_bytes: u64,
}

pub(crate) fn write<R: Read + Seek + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    parsed: &ParsedWave,
    options: &Options,
) -> Result<WriteStats, Error> {
    let input_width = (parsed.format.bits_per_sample / 8) as usize;
    let output_width = (parsed.format.valid_bits_per_sample as usize).div_ceil(8);
    let output_block_align = (parsed.format.channels as u64)
        .checked_mul(output_width as u64)
        .ok_or(Error::IntegerOverflow("AIFF output block align"))?;
    let output_audio_bytes = (parsed.sample_frames as u64)
        .checked_mul(output_block_align)
        .ok_or(Error::IntegerOverflow("AIFF audio size"))?;
    let ssnd_size = 8_u64
        .checked_add(output_audio_bytes)
        .ok_or(Error::IntegerOverflow("SSND size"))?;
    if ssnd_size > u32::MAX as u64 {
        return Err(Error::AiffSizeLimitExceeded {
            bytes: ssnd_size + 8,
        });
    }
    let ssnd_total = chunk_total(ssnd_size)?;
    let chunks_size = chunk_total(18)?
        .checked_add(metadata_total(&parsed.metadata)?)
        .and_then(|value| value.checked_add(ssnd_total))
        .ok_or(Error::IntegerOverflow("AIFF FORM size"))?;
    let form_size = 4_u64
        .checked_add(chunks_size)
        .ok_or(Error::IntegerOverflow("AIFF FORM payload"))?;
    if form_size > u32::MAX as u64 {
        return Err(Error::AiffSizeLimitExceeded {
            bytes: form_size + 8,
        });
    }

    let mut output = CountingWriter {
        inner: output,
        count: 0,
    };
    output.write_all(b"FORM")?;
    output.write_all(&(form_size as u32).to_be_bytes())?;
    output.write_all(b"AIFF")?;

    let mut common = [0_u8; 18];
    common[0..2].copy_from_slice(&parsed.format.channels.to_be_bytes());
    common[2..6].copy_from_slice(&parsed.sample_frames.to_be_bytes());
    common[6..8].copy_from_slice(&parsed.format.valid_bits_per_sample.to_be_bytes());
    common[8..18].copy_from_slice(&extended_sample_rate(parsed.format.sample_rate));
    write_chunk(&mut output, b"COMM", &common)?;
    for item in &parsed.metadata {
        write_chunk(&mut output, &item.id, &item.data)?;
    }

    output.write_all(b"SSND")?;
    output.write_all(&(ssnd_size as u32).to_be_bytes())?;
    output.write_all(&0_u32.to_be_bytes())?; // offset
    output.write_all(&0_u32.to_be_bytes())?; // block size

    let buffer_len = options.buffer_size - (options.buffer_size % input_width);
    if buffer_len == 0 {
        return Err(Error::InvalidOptions("buffer_size is smaller than one input sample"));
    }
    let mut source = vec![0_u8; buffer_len];
    let mut transformed = Vec::with_capacity(buffer_len);
    for segment in &parsed.data_segments {
        input.seek(SeekFrom::Start(segment.offset))?;
        let mut remaining = segment.size;
        while remaining != 0 {
            let take = usize::try_from(remaining.min(buffer_len as u64))
                .map_err(|_| Error::IntegerOverflow("PCM read length"))?;
            input.read_exact(&mut source[..take]).map_err(|error| {
                if error.kind() == std::io::ErrorKind::UnexpectedEof {
                    Error::Truncated("PCM data")
                } else {
                    Error::Io(error)
                }
            })?;
            transform_samples(&source[..take], input_width, output_width, &mut transformed);
            output.write_all(&transformed)?;
            remaining -= take as u64;
        }
    }
    if output_audio_bytes & 1 != 0 {
        output.write_all(&[0])?;
    }
    output.flush()?;
    debug_assert_eq!(output.count, form_size + 8);
    Ok(WriteStats {
        output_bytes: output.count,
        output_audio_bytes,
    })
}
