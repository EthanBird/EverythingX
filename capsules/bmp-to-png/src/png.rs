use std::io::{Read, Seek, Write};

use crate::bmp::Image;
use crate::zlib::{FixedRleDeflater, StoredDeflater};
use crate::{CompressionStrategy, Error, FilterStrategy, Options};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1A\n";

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                0xEDB88320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

const CRC_TABLE: [u32; 256] = make_crc_table();

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        let index = ((crc ^ *byte as u32) & 0xFF) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    crc
}

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

fn write_chunk<W: Write + ?Sized>(
    output: &mut W,
    chunk_type: &[u8; 4],
    data: &[u8],
) -> Result<(), Error> {
    let length = u32::try_from(data.len())
        .map_err(|_| Error::IntegerOverflow("PNG chunk length"))?;
    output.write_all(&length.to_be_bytes())?;
    output.write_all(chunk_type)?;
    output.write_all(data)?;
    let mut crc = crc32_update(u32::MAX, chunk_type);
    crc = crc32_update(crc, data) ^ u32::MAX;
    output.write_all(&crc.to_be_bytes())?;
    Ok(())
}

struct IdatWriter<'a, W: Write + ?Sized> {
    output: &'a mut W,
    buffer: Vec<u8>,
    chunk_size: usize,
}

impl<'a, W: Write + ?Sized> IdatWriter<'a, W> {
    fn new(output: &'a mut W, chunk_size: usize) -> Self {
        Self {
            output,
            buffer: Vec::with_capacity(chunk_size),
            chunk_size,
        }
    }

    fn flush_chunk(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        write_chunk(self.output, b"IDAT", &self.buffer)
            .map_err(|error| match error {
                Error::Io(error) => error,
                other => std::io::Error::other(other.to_string()),
            })?;
        self.buffer.clear();
        Ok(())
    }

    fn finish(mut self) -> Result<(), Error> {
        self.flush_chunk()?;
        Ok(())
    }
}

impl<W: Write + ?Sized> Write for IdatWriter<'_, W> {
    fn write(&mut self, mut bytes: &[u8]) -> std::io::Result<usize> {
        let original = bytes.len();
        while !bytes.is_empty() {
            if self.buffer.len() == self.chunk_size {
                self.flush_chunk()?;
            }
            let available = self.chunk_size - self.buffer.len();
            let take = available.min(bytes.len());
            self.buffer.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
        }
        Ok(original)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_chunk()?;
        self.output.flush()
    }
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let left = left as i32;
    let above = above as i32;
    let upper_left = upper_left as i32;
    let estimate = left + above - upper_left;
    let left_distance = (estimate - left).abs();
    let above_distance = (estimate - above).abs();
    let diagonal_distance = (estimate - upper_left).abs();
    if left_distance <= above_distance && left_distance <= diagonal_distance {
        left as u8
    } else if above_distance <= diagonal_distance {
        above as u8
    } else {
        upper_left as u8
    }
}

fn filter_code(strategy: FilterStrategy) -> u8 {
    match strategy {
        FilterStrategy::None => 0,
        FilterStrategy::Sub => 1,
        FilterStrategy::Up => 2,
        FilterStrategy::Average => 3,
        FilterStrategy::Paeth => 4,
        FilterStrategy::Adaptive => unreachable!(),
    }
}

fn apply_filter(
    code: u8,
    raw: &[u8],
    previous: &[u8],
    bytes_per_pixel: usize,
    output: &mut Vec<u8>,
) {
    output.clear();
    output.reserve(raw.len() + 1);
    output.push(code);
    for index in 0..raw.len() {
        let left = if index >= bytes_per_pixel {
            raw[index - bytes_per_pixel]
        } else {
            0
        };
        let above = previous[index];
        let upper_left = if index >= bytes_per_pixel {
            previous[index - bytes_per_pixel]
        } else {
            0
        };
        let predictor = match code {
            0 => 0,
            1 => left,
            2 => above,
            3 => ((left as u16 + above as u16) / 2) as u8,
            4 => paeth(left, above, upper_left),
            _ => unreachable!(),
        };
        output.push(raw[index].wrapping_sub(predictor));
    }
}

fn filter_score(filtered: &[u8]) -> u64 {
    filtered[1..]
        .iter()
        .map(|value| {
            let magnitude = (*value as i8 as i16).unsigned_abs();
            magnitude as u64
        })
        .sum()
}

fn filter_row(
    strategy: FilterStrategy,
    raw: &[u8],
    previous: &[u8],
    bytes_per_pixel: usize,
    best: &mut Vec<u8>,
    candidate: &mut Vec<u8>,
) {
    if strategy != FilterStrategy::Adaptive {
        apply_filter(filter_code(strategy), raw, previous, bytes_per_pixel, best);
        return;
    }
    let mut best_score = u64::MAX;
    for code in 0..=4 {
        apply_filter(code, raw, previous, bytes_per_pixel, candidate);
        let score = filter_score(candidate);
        if score < best_score {
            best_score = score;
            std::mem::swap(best, candidate);
        }
    }
}

pub(crate) struct Stats {
    pub(crate) output_bytes: u64,
}

pub(crate) fn encode<R: Read + Seek + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    image: &Image,
    options: &Options,
) -> Result<Stats, Error> {
    let mut output = CountingWriter {
        inner: output,
        count: 0,
    };
    output.write_all(PNG_SIGNATURE)?;
    let mut ihdr = [0_u8; 13];
    ihdr[0..4].copy_from_slice(&image.width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&image.height.to_be_bytes());
    ihdr[8] = 8;
    ihdr[9] = if image.output_channels == 4 { 6 } else { 2 };
    write_chunk(&mut output, b"IHDR", &ihdr)?;

    {
        let mut idat = IdatWriter::new(&mut output, options.idat_chunk_size);
        let row_len = (image.width as usize)
            .checked_mul(image.output_channels)
            .ok_or(Error::IntegerOverflow("PNG row length"))?;
        let mut previous = vec![0_u8; row_len];
        let mut decoded = Vec::with_capacity(row_len);
        let mut filtered = Vec::with_capacity(row_len + 1);
        let mut candidate = Vec::with_capacity(row_len + 1);

        match options.compression {
            CompressionStrategy::Store => {
                let mut deflater = StoredDeflater::new(&mut idat)?;
                for row in 0..image.height {
                    image.decode_row(input, row, &mut decoded)?;
                    filter_row(
                        options.filter,
                        &decoded,
                        &previous,
                        image.output_channels,
                        &mut filtered,
                        &mut candidate,
                    );
                    deflater.write_bytes(&filtered)?;
                    previous.copy_from_slice(&decoded);
                }
                deflater.finish()?;
            }
            CompressionStrategy::FixedRle => {
                let mut deflater = FixedRleDeflater::new(&mut idat)?;
                for row in 0..image.height {
                    image.decode_row(input, row, &mut decoded)?;
                    filter_row(
                        options.filter,
                        &decoded,
                        &previous,
                        image.output_channels,
                        &mut filtered,
                        &mut candidate,
                    );
                    deflater.write_bytes(&filtered)?;
                    previous.copy_from_slice(&decoded);
                }
                deflater.finish()?;
            }
        }
        idat.finish()?;
    }

    write_chunk(&mut output, b"IEND", &[])?;
    output.flush()?;
    Ok(Stats {
        output_bytes: output.count,
    })
}

