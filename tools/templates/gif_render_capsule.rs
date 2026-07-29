#![forbid(unsafe_code)]

mod legacy_native;
mod png_native;

use std::fmt;
use std::io::{self, Read, Write};

const OPERATION: Operation = Operation::__OPERATION__;
const DEFAULT_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Frame,
    SpriteSheet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub max_input_bytes: u64,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub frame_index: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_pixels: DEFAULT_MAX_PIXELS,
            max_frames: 10_000,
            frame_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub source_width: u32,
    pub source_height: u32,
    pub source_frames: u32,
    pub width: u32,
    pub height: u32,
    pub selected_frame: Option<u32>,
    pub peak_working_memory_bytes: u64,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    InputTooLarge { bytes: u64, limit: u64 },
    PixelLimitExceeded { pixels: u64, limit: u64 },
    InvalidInput(String),
    IntegerOverflow(&'static str),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(value) => write!(f, "invalid options: {value}"),
            Self::InputTooLarge { bytes, limit } => {
                write!(f, "input has {bytes} bytes, exceeding {limit}")
            }
            Self::PixelLimitExceeded { pixels, limit } => {
                write!(f, "image has {pixels} pixels, exceeding {limit}")
            }
            Self::InvalidInput(value) => write!(f, "invalid GIF input: {value}"),
            Self::IntegerOverflow(value) => write!(f, "integer overflow while computing {value}"),
            Self::Io(value) => fmt::Display::fmt(value, f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(value) => Some(value),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn png_image(image: legacy_native::Image) -> png_native::Image {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    png_native::Image {
        width: image.width,
        height: image.height,
        source_channels: if alpha { 4 } else { 3 },
        source_bit_depth: 8,
        source_color_type: if alpha { 6 } else { 2 },
        interlaced: false,
        pixels: image
            .pixels
            .into_iter()
            .map(|pixel| png_native::Pixel16 {
                r: pixel.r as u16 * 257,
                g: pixel.g as u16 * 257,
                b: pixel.b as u16 * 257,
                a: pixel.a as u16 * 257,
            })
            .collect(),
        warnings: Vec::new(),
    }
}

pub fn convert<R: Read + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.max_input_bytes == 0 || options.max_pixels == 0 || options.max_frames == 0 {
        return Err(Error::InvalidOptions("resource limits must be non-zero"));
    }
    let mut source = Vec::new();
    input
        .take(options.max_input_bytes.saturating_add(1))
        .read_to_end(&mut source)?;
    if source.len() as u64 > options.max_input_bytes {
        return Err(Error::InputTooLarge {
            bytes: source.len() as u64,
            limit: options.max_input_bytes,
        });
    }
    let file = legacy_native::decode_gif(
        &source,
        options.max_pixels,
        options.max_frames,
        usize::try_from(options.max_input_bytes)
            .map_err(|_| Error::IntegerOverflow("GIF sub-block limit"))?,
    )
    .map_err(|error| match error {
        legacy_native::Error::Limit("pixel count") => Error::PixelLimitExceeded {
            pixels: options.max_pixels.saturating_add(1),
            limit: options.max_pixels,
        },
        other => Error::InvalidInput(other.to_string()),
    })?;
    let source_frames = file.frames.len() as u32;
    let (target, selected_frame) = match OPERATION {
        Operation::Frame => {
            let frame = file
                .frames
                .get(options.frame_index as usize)
                .ok_or(Error::InvalidOptions("frame_index is outside the GIF frame set"))?;
            (frame.image.clone(), Some(options.frame_index))
        }
        Operation::SpriteSheet => {
            let width = file
                .width
                .checked_mul(source_frames)
                .ok_or(Error::IntegerOverflow("sprite-sheet width"))?;
            let pixels = (width as u64)
                .checked_mul(file.height as u64)
                .ok_or(Error::IntegerOverflow("sprite-sheet pixels"))?;
            if pixels > options.max_pixels {
                return Err(Error::PixelLimitExceeded {
                    pixels,
                    limit: options.max_pixels,
                });
            }
            let count =
                usize::try_from(pixels).map_err(|_| Error::IntegerOverflow("sprite-sheet allocation"))?;
            let mut target = vec![legacy_native::Pixel::default(); count];
            for (frame_index, frame) in file.frames.iter().enumerate() {
                for y in 0..file.height as usize {
                    let source_start = y * file.width as usize;
                    let target_start =
                        y * width as usize + frame_index * file.width as usize;
                    target[target_start..target_start + file.width as usize].copy_from_slice(
                        &frame.image.pixels[source_start..source_start + file.width as usize],
                    );
                }
            }
            (
                legacy_native::Image { width, height: file.height, pixels: target },
                None,
            )
        }
    };
    let target_width = target.width;
    let target_height = target.height;
    let pixel_memory = target.pixels.len() as u64 * 4;
    let png = png_native::encode(&png_image(target), png_native::Filter::Adaptive)
        .map_err(|error| Error::InvalidInput(error.to_string()))?;
    output.write_all(&png)?;
    Ok(Report {
        input_bytes: source.len() as u64,
        output_bytes: png.len() as u64,
        source_width: file.width,
        source_height: file.height,
        source_frames,
        width: target_width,
        height: target_height,
        selected_frame,
        peak_working_memory_bytes: source.len() as u64 + pixel_memory + png.len() as u64,
        strategy: match OPERATION {
            Operation::Frame => "gif-composited-frame-selection",
            Operation::SpriteSheet => "gif-horizontal-composited-sprite-sheet",
        },
        backend: "native-portable",
        warnings: Vec::new(),
    })
}

fn animation_fixture() -> Vec<u8> {
    let image = legacy_native::Image {
        width: 3,
        height: 2,
        pixels: vec![
            legacy_native::Pixel { r: 0, g: 0, b: 0, a: 255 },
            legacy_native::Pixel { r: 255, g: 0, b: 0, a: 255 },
            legacy_native::Pixel { r: 0, g: 255, b: 0, a: 255 },
            legacy_native::Pixel { r: 0, g: 0, b: 255, a: 255 },
            legacy_native::Pixel { r: 255, g: 255, b: 0, a: 255 },
            legacy_native::Pixel { r: 255, g: 255, b: 255, a: 255 },
        ],
    };
    let mut gif = legacy_native::encode_gif(&image).expect("fixture GIF");
    let table_entries = 1usize << ((gif[10] & 7) + 1);
    let image_start = 13 + table_entries * 3;
    let frame = gif[image_start..gif.len() - 1].to_vec();
    gif.pop();
    gif.extend_from_slice(&frame);
    gif.push(0x3b);
    gif
}

#[doc(hidden)]
pub fn conformance_fixture() -> Vec<u8> {
    animation_fixture()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_render_valid_png() {
        let source = conformance_fixture();
        let mut output = Vec::new();
        let report = convert(&mut &source[..], &mut output, &Options::default()).unwrap();
        let decoded = png_native::decode(
            &output,
            &png_native::DecodeOptions {
                max_pixels: 100,
                max_inflate_bytes: 10_000,
                strict_crc: true,
                strict_trailing_data: true,
            },
        )
        .unwrap();
        assert_eq!(report.source_frames, 2);
        assert_eq!(decoded.width, report.width);
        assert_eq!(decoded.height, report.height);
    }

    #[test]
    fn sprite_sheet_contains_every_frame() {
        if OPERATION != Operation::SpriteSheet {
            return;
        }
        let source = conformance_fixture();
        let report =
            convert(&mut &source[..], &mut Vec::new(), &Options::default()).unwrap();
        assert_eq!(report.width, report.source_width * 2);
    }

    #[test]
    fn frame_index_is_checked() {
        if OPERATION != Operation::Frame {
            return;
        }
        let source = conformance_fixture();
        let mut options = Options::default();
        options.frame_index = 2;
        assert!(matches!(
            convert(&mut &source[..], &mut Vec::new(), &options),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[test]
    fn truncation_is_rejected_before_output() {
        let source = conformance_fixture();
        let mut output = Vec::new();
        assert!(convert(
            &mut &source[..source.len() - 1],
            &mut output,
            &Options::default()
        )
        .is_err());
        assert!(output.is_empty());
    }
}
