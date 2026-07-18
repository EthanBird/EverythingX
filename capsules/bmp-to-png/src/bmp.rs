use std::io::{Read, Seek, SeekFrom};

use crate::{Error, Options, UnmarkedAlpha};

const BI_RGB: u32 = 0;
const BI_RLE8: u32 = 1;
const BI_RLE4: u32 = 2;
const BI_BITFIELDS: u32 = 3;
const BI_ALPHABITFIELDS: u32 = 6;
const MAX_DIB_HEADER: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Compression {
    Rgb,
    Rle8,
    Rle4,
    Bitfields,
    AlphaBitfields,
}

impl Compression {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Rgb => "BI_RGB",
            Self::Rle8 => "BI_RLE8",
            Self::Rle4 => "BI_RLE4",
            Self::Bitfields => "BI_BITFIELDS",
            Self::AlphaBitfields => "BI_ALPHABITFIELDS",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Masks {
    red: u32,
    green: u32,
    blue: u32,
    alpha: u32,
}

pub(crate) struct Image {
    pub(crate) file_size: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bits_per_pixel: u16,
    pub(crate) compression: Compression,
    pub(crate) top_down: bool,
    pub(crate) pixel_offset: u64,
    pub(crate) row_stride: u64,
    pub(crate) palette: Vec<[u8; 4]>,
    masks: Masks,
    preserve_unmarked_alpha: bool,
    pub(crate) output_channels: usize,
    pub(crate) rle_pixels: Option<Vec<u8>>,
    pub(crate) warnings: Vec<String>,
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn le_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
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

fn valid_mask(mask: u32) -> bool {
    if mask == 0 {
        return false;
    }
    let shifted = mask >> mask.trailing_zeros();
    shifted & shifted.wrapping_add(1) == 0
}

fn validate_masks(masks: Masks, bits_per_pixel: u16) -> Result<(), Error> {
    if !valid_mask(masks.red) || !valid_mask(masks.green) || !valid_mask(masks.blue) {
        return Err(Error::InvalidBitfieldMasks("RGB masks must be non-zero contiguous bit ranges"));
    }
    if masks.alpha != 0 && !valid_mask(masks.alpha) {
        return Err(Error::InvalidBitfieldMasks("alpha mask must be a contiguous bit range"));
    }
    let overlap = (masks.red & masks.green)
        | (masks.red & masks.blue)
        | (masks.green & masks.blue)
        | (masks.alpha & (masks.red | masks.green | masks.blue));
    if overlap != 0 {
        return Err(Error::InvalidBitfieldMasks("channel masks overlap"));
    }
    let allowed = if bits_per_pixel == 32 {
        u32::MAX
    } else {
        (1_u32 << bits_per_pixel) - 1
    };
    if (masks.red | masks.green | masks.blue | masks.alpha) & !allowed != 0 {
        return Err(Error::InvalidBitfieldMasks("mask uses bits outside the pixel word"));
    }
    Ok(())
}

pub(crate) fn inspect<R: Read + Seek + ?Sized>(
    input: &mut R,
    options: &Options,
) -> Result<Image, Error> {
    let file_size = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;

    let mut file_header = [0_u8; 14];
    read_exact(input, &mut file_header, "file header")?;
    if &file_header[0..2] != b"BM" {
        return Err(Error::InvalidSignature);
    }
    let declared_file_size = le_u32(&file_header[2..6]);
    let pixel_offset = le_u32(&file_header[10..14]);
    if options.strict_declared_file_size
        && declared_file_size != 0
        && declared_file_size as u64 > file_size
    {
        return Err(Error::DeclaredFileSizeExceedsInput {
            declared: declared_file_size,
            actual: file_size,
        });
    }

    let mut size_bytes = [0_u8; 4];
    read_exact(input, &mut size_bytes, "DIB header")?;
    let dib_size = le_u32(&size_bytes);
    if !(40..=MAX_DIB_HEADER).contains(&dib_size) {
        return Err(Error::UnsupportedDibHeader(dib_size));
    }
    let remaining_header = usize::try_from(dib_size - 4)
        .map_err(|_| Error::IntegerOverflow("DIB header allocation"))?;
    let mut dib = vec![0_u8; remaining_header];
    read_exact(input, &mut dib, "DIB header")?;

    let signed_width = le_i32(&dib[0..4]);
    let signed_height = le_i32(&dib[4..8]);
    if signed_width <= 0 || signed_height == 0 || signed_height == i32::MIN {
        return Err(Error::InvalidDimensions {
            width: signed_width,
            height: signed_height,
        });
    }
    let width = signed_width as u32;
    let height = signed_height.unsigned_abs();
    let pixels = (width as u64)
        .checked_mul(height as u64)
        .ok_or(Error::IntegerOverflow("pixel count"))?;
    if pixels > options.max_pixels {
        return Err(Error::PixelLimitExceeded {
            pixels,
            limit: options.max_pixels,
        });
    }
    let planes = le_u16(&dib[8..10]);
    if planes != 1 {
        return Err(Error::UnsupportedPlanes(planes));
    }
    let bits_per_pixel = le_u16(&dib[10..12]);
    if !matches!(bits_per_pixel, 1 | 4 | 8 | 16 | 24 | 32) {
        return Err(Error::UnsupportedBitDepth(bits_per_pixel));
    }
    let compression_raw = le_u32(&dib[12..16]);
    let compression = match compression_raw {
        BI_RGB => Compression::Rgb,
        BI_RLE8 if bits_per_pixel == 8 => Compression::Rle8,
        BI_RLE4 if bits_per_pixel == 4 => Compression::Rle4,
        BI_BITFIELDS if matches!(bits_per_pixel, 16 | 32) => Compression::Bitfields,
        BI_ALPHABITFIELDS if matches!(bits_per_pixel, 16 | 32) => Compression::AlphaBitfields,
        _ => {
            return Err(Error::UnsupportedCompression {
                compression: compression_raw,
                bits_per_pixel,
            });
        }
    };
    let top_down = signed_height < 0;
    if top_down && matches!(compression, Compression::Rle4 | Compression::Rle8) {
        return Err(Error::InvalidRle("top-down RLE BMP is not defined"));
    }
    let image_size = le_u32(&dib[16..20]);
    let colors_used = le_u32(&dib[28..32]);

    let mut masks = match (compression, bits_per_pixel) {
        (Compression::Rgb, 16) => Masks {
            red: 0x7C00,
            green: 0x03E0,
            blue: 0x001F,
            alpha: 0,
        },
        (Compression::Rgb, 32) => Masks {
            red: 0x00FF0000,
            green: 0x0000FF00,
            blue: 0x000000FF,
            alpha: 0,
        },
        _ => Masks {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        },
    };

    if matches!(compression, Compression::Bitfields | Compression::AlphaBitfields) {
        if dib_size >= 52 {
            masks.red = le_u32(&dib[36..40]);
            masks.green = le_u32(&dib[40..44]);
            masks.blue = le_u32(&dib[44..48]);
            if dib_size >= 56 {
                masks.alpha = le_u32(&dib[48..52]);
            }
        } else {
            let mask_count = if compression == Compression::AlphaBitfields { 4 } else { 3 };
            let mut external = [0_u8; 16];
            read_exact(input, &mut external[..mask_count * 4], "external bitfield masks")?;
            masks.red = le_u32(&external[0..4]);
            masks.green = le_u32(&external[4..8]);
            masks.blue = le_u32(&external[8..12]);
            if mask_count == 4 {
                masks.alpha = le_u32(&external[12..16]);
            }
        }
        validate_masks(masks, bits_per_pixel)?;
    }

    let palette_limit = if bits_per_pixel <= 8 {
        1_u32 << bits_per_pixel
    } else {
        0
    };
    let palette_count = if palette_limit == 0 {
        0
    } else if colors_used == 0 {
        palette_limit
    } else if colors_used <= palette_limit {
        colors_used
    } else {
        return Err(Error::InvalidPalette("colors_used exceeds the bit-depth palette limit"));
    };
    let position_before_palette = input.stream_position()?;
    let palette_bytes = (palette_count as u64)
        .checked_mul(4)
        .ok_or(Error::IntegerOverflow("palette size"))?;
    if position_before_palette
        .checked_add(palette_bytes)
        .ok_or(Error::IntegerOverflow("palette end"))?
        > pixel_offset as u64
    {
        return Err(Error::InvalidPixelOffset(pixel_offset));
    }
    let mut palette = Vec::with_capacity(palette_count as usize);
    for _ in 0..palette_count {
        let mut entry = [0_u8; 4];
        read_exact(input, &mut entry, "palette")?;
        palette.push([entry[2], entry[1], entry[0], entry[3]]);
    }

    let row_bits = (width as u64)
        .checked_mul(bits_per_pixel as u64)
        .ok_or(Error::IntegerOverflow("row bits"))?;
    let row_stride = row_bits
        .checked_add(31)
        .map(|value| (value / 32) * 4)
        .ok_or(Error::IntegerOverflow("row stride"))?;
    if pixel_offset as u64 >= file_size {
        return Err(Error::InvalidPixelOffset(pixel_offset));
    }
    if matches!(compression, Compression::Rgb | Compression::Bitfields | Compression::AlphaBitfields) {
        let pixel_end = (pixel_offset as u64)
            .checked_add(
                row_stride
                    .checked_mul(height as u64)
                    .ok_or(Error::IntegerOverflow("pixel array size"))?,
            )
            .ok_or(Error::IntegerOverflow("pixel array end"))?;
        if pixel_end > file_size {
            return Err(Error::Truncated("pixel array"));
        }
    } else if image_size != 0
        && (pixel_offset as u64)
            .checked_add(image_size as u64)
            .ok_or(Error::IntegerOverflow("RLE stream end"))?
            > file_size
    {
        return Err(Error::Truncated("RLE pixel array"));
    }

    let preserve_unmarked_alpha = options.unmarked_alpha == UnmarkedAlpha::Preserve;
    let output_channels = if masks.alpha != 0
        || (preserve_unmarked_alpha && (bits_per_pixel == 32 || !palette.is_empty()))
    {
        4
    } else {
        3
    };
    let mut warnings = Vec::new();
    if declared_file_size != 0 && declared_file_size as u64 != file_size {
        warnings.push(format!(
            "declared BMP file size {declared_file_size} differs from actual size {file_size}"
        ));
    }
    if bits_per_pixel == 32 && masks.alpha == 0 && !preserve_unmarked_alpha {
        warnings.push("unmarked 32-bit BMP alpha byte was normalized to opaque".into());
    }

    let rle_pixels = if matches!(compression, Compression::Rle4 | Compression::Rle8) {
        Some(decode_rle(
            input,
            pixel_offset as u64,
            if image_size == 0 { file_size - pixel_offset as u64 } else { image_size as u64 },
            width,
            height,
            compression,
        )?)
    } else {
        None
    };

    Ok(Image {
        file_size,
        width,
        height,
        bits_per_pixel,
        compression,
        top_down,
        pixel_offset: pixel_offset as u64,
        row_stride,
        palette,
        masks,
        preserve_unmarked_alpha,
        output_channels,
        rle_pixels,
        warnings,
    })
}

struct RleReader<'a, R: Read + ?Sized> {
    input: &'a mut R,
    remaining: u64,
}

impl<R: Read + ?Sized> RleReader<'_, R> {
    fn byte(&mut self) -> Result<u8, Error> {
        if self.remaining == 0 {
            return Err(Error::Truncated("RLE command stream"));
        }
        let mut value = [0_u8; 1];
        read_exact(self.input, &mut value, "RLE command stream")?;
        self.remaining -= 1;
        Ok(value[0])
    }
}

fn set_index(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: &mut u32,
    y_from_bottom: u32,
    value: u8,
) -> Result<(), Error> {
    if *x >= width || y_from_bottom >= height {
        return Err(Error::InvalidRle("pixel run exceeds image bounds"));
    }
    let top_row = height - 1 - y_from_bottom;
    let index = (top_row as usize)
        .checked_mul(width as usize)
        .and_then(|base| base.checked_add(*x as usize))
        .ok_or(Error::IntegerOverflow("RLE pixel index"))?;
    pixels[index] = value;
    *x += 1;
    Ok(())
}

fn decode_rle<R: Read + Seek + ?Sized>(
    input: &mut R,
    offset: u64,
    length: u64,
    width: u32,
    height: u32,
    compression: Compression,
) -> Result<Vec<u8>, Error> {
    input.seek(SeekFrom::Start(offset))?;
    let allocation = (width as usize)
        .checked_mul(height as usize)
        .ok_or(Error::IntegerOverflow("RLE image allocation"))?;
    let mut pixels = vec![0_u8; allocation];
    let mut reader = RleReader { input, remaining: length };
    let mut x = 0_u32;
    let mut y = 0_u32;

    loop {
        let count = reader.byte()?;
        let value = reader.byte()?;
        if count != 0 {
            for index in 0..count {
                let palette_index = if compression == Compression::Rle8 {
                    value
                } else if index % 2 == 0 {
                    value >> 4
                } else {
                    value & 0x0F
                };
                set_index(&mut pixels, width, height, &mut x, y, palette_index)?;
            }
            continue;
        }

        match value {
            0 => {
                x = 0;
                y = y.checked_add(1).ok_or(Error::IntegerOverflow("RLE row"))?;
                if y > height {
                    return Err(Error::InvalidRle("too many end-of-line commands"));
                }
            }
            1 => return Ok(pixels),
            2 => {
                let dx = reader.byte()? as u32;
                let dy = reader.byte()? as u32;
                x = x.checked_add(dx).ok_or(Error::IntegerOverflow("RLE x delta"))?;
                y = y.checked_add(dy).ok_or(Error::IntegerOverflow("RLE y delta"))?;
                if x > width || y >= height {
                    return Err(Error::InvalidRle("delta command leaves image bounds"));
                }
            }
            literal_count => {
                if y >= height {
                    return Err(Error::InvalidRle("absolute run follows final row"));
                }
                if compression == Compression::Rle8 {
                    for _ in 0..literal_count {
                        let index = reader.byte()?;
                        set_index(&mut pixels, width, height, &mut x, y, index)?;
                    }
                    if literal_count % 2 != 0 {
                        let _padding = reader.byte()?;
                    }
                } else {
                    let encoded_bytes = (literal_count as usize).div_ceil(2);
                    let mut packed = vec![0_u8; encoded_bytes];
                    for byte in &mut packed {
                        *byte = reader.byte()?;
                    }
                    for pixel_index in 0..literal_count as usize {
                        let source = packed[pixel_index / 2];
                        let value = if pixel_index % 2 == 0 {
                            source >> 4
                        } else {
                            source & 0x0F
                        };
                        set_index(&mut pixels, width, height, &mut x, y, value)?;
                    }
                    if encoded_bytes % 2 != 0 {
                        let _padding = reader.byte()?;
                    }
                }
            }
        }
    }
}

fn scale_mask(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 255;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    let channel = (value & mask) >> shift;
    ((channel as u64 * 255 + maximum as u64 / 2) / maximum as u64) as u8
}

impl Image {
    pub(crate) fn decode_row<R: Read + Seek + ?Sized>(
        &self,
        input: &mut R,
        row_index: u32,
        decoded: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let decoded_len = (self.width as usize)
            .checked_mul(self.output_channels)
            .ok_or(Error::IntegerOverflow("decoded row allocation"))?;
        decoded.clear();
        decoded.resize(decoded_len, 0);

        let mut packed = vec![0_u8; self.row_stride as usize];
        if let Some(indices) = &self.rle_pixels {
            let start = (row_index as usize)
                .checked_mul(self.width as usize)
                .ok_or(Error::IntegerOverflow("RLE row offset"))?;
            self.decode_palette_indices(&indices[start..start + self.width as usize], decoded)?;
            return Ok(());
        }

        let source_row = if self.top_down {
            row_index
        } else {
            self.height - 1 - row_index
        };
        let offset = self
            .pixel_offset
            .checked_add(
                self.row_stride
                    .checked_mul(source_row as u64)
                    .ok_or(Error::IntegerOverflow("source row offset"))?,
            )
            .ok_or(Error::IntegerOverflow("source row position"))?;
        input.seek(SeekFrom::Start(offset))?;
        read_exact(input, &mut packed, "pixel row")?;

        if self.bits_per_pixel <= 8 {
            let mut indices = vec![0_u8; self.width as usize];
            for x in 0..self.width as usize {
                indices[x] = match self.bits_per_pixel {
                    1 => (packed[x / 8] >> (7 - x % 8)) & 1,
                    4 => {
                        if x % 2 == 0 { packed[x / 2] >> 4 } else { packed[x / 2] & 0x0F }
                    }
                    8 => packed[x],
                    _ => unreachable!(),
                };
            }
            self.decode_palette_indices(&indices, decoded)?;
            return Ok(());
        }

        for x in 0..self.width as usize {
            let target = x * self.output_channels;
            match self.bits_per_pixel {
                16 => {
                    let source = x * 2;
                    let value = le_u16(&packed[source..source + 2]) as u32;
                    decoded[target] = scale_mask(value, self.masks.red);
                    decoded[target + 1] = scale_mask(value, self.masks.green);
                    decoded[target + 2] = scale_mask(value, self.masks.blue);
                    if self.output_channels == 4 {
                        decoded[target + 3] = scale_mask(value, self.masks.alpha);
                    }
                }
                24 => {
                    let source = x * 3;
                    decoded[target] = packed[source + 2];
                    decoded[target + 1] = packed[source + 1];
                    decoded[target + 2] = packed[source];
                }
                32 => {
                    let source = x * 4;
                    let value = le_u32(&packed[source..source + 4]);
                    decoded[target] = scale_mask(value, self.masks.red);
                    decoded[target + 1] = scale_mask(value, self.masks.green);
                    decoded[target + 2] = scale_mask(value, self.masks.blue);
                    if self.output_channels == 4 {
                        decoded[target + 3] = if self.masks.alpha != 0 {
                            scale_mask(value, self.masks.alpha)
                        } else if self.preserve_unmarked_alpha {
                            packed[source + 3]
                        } else {
                            255
                        };
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    fn decode_palette_indices(&self, indices: &[u8], decoded: &mut [u8]) -> Result<(), Error> {
        for (x, index) in indices.iter().copied().enumerate() {
            let entry = self.palette.get(index as usize).ok_or(Error::PaletteIndexOutOfRange {
                index,
                entries: self.palette.len(),
            })?;
            let target = x * self.output_channels;
            decoded[target] = entry[0];
            decoded[target + 1] = entry[1];
            decoded[target + 2] = entry[2];
            if self.output_channels == 4 {
                decoded[target + 3] = if self.preserve_unmarked_alpha { entry[3] } else { 255 };
            }
        }
        Ok(())
    }
}
