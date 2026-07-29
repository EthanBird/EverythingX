#![forbid(unsafe_code)]

mod legacy_native;
mod png_native;

use std::fmt;
use std::io::{self, Read, Write};

const FORMAT: Format = Format::Ico;
const DEFAULT_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Gif,
    Ico,
    Cur,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub max_input_bytes: u64,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub max_members: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_pixels: DEFAULT_MAX_PIXELS,
            max_frames: 10_000,
            max_members: 1_024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub frames_or_members: u32,
    pub png_members: u32,
    pub dib_members: u32,
    pub peak_working_memory_bytes: u64,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    InputTooLarge { bytes: u64, limit: u64 },
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
            Self::InvalidInput(value) => write!(f, "invalid input: {value}"),
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

fn validate_member(
    member: &legacy_native::IconMember<'_>,
    options: &Options,
) -> Result<u64, Error> {
    let image_bytes = if member.png {
        let decoded = png_native::decode(
            member.payload,
            &png_native::DecodeOptions {
                max_pixels: options.max_pixels,
                max_inflate_bytes: options.max_input_bytes,
                strict_crc: true,
                strict_trailing_data: true,
            },
        )
        .map_err(|error| Error::InvalidInput(error.to_string()))?;
        if decoded.width != member.width || decoded.height != member.height {
            return Err(Error::InvalidInput(
                "ICO/CUR directory dimensions disagree with PNG member".into(),
            ));
        }
        decoded.pixels.len() as u64 * 8
    } else {
        let decoded = legacy_native::decode_dib(member.payload, options.max_pixels)
            .map_err(|error| Error::InvalidInput(error.to_string()))?;
        if decoded.width != member.width || decoded.height != member.height {
            return Err(Error::InvalidInput(
                "ICO/CUR directory dimensions disagree with DIB member".into(),
            ));
        }
        decoded.pixels.len() as u64 * 4
    };
    Ok(image_bytes)
}

pub fn convert<R: Read + ?Sized, W: Write + ?Sized>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.max_input_bytes == 0
        || options.max_pixels == 0
        || options.max_frames == 0
        || options.max_members == 0
    {
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
    let (width, height, count, png_members, dib_members, decoded_memory) = match FORMAT {
        Format::Gif => {
            let file = legacy_native::decode_gif(
                &source,
                options.max_pixels,
                options.max_frames,
                usize::try_from(options.max_input_bytes)
                    .map_err(|_| Error::IntegerOverflow("GIF sub-block limit"))?,
            )
            .map_err(|error| Error::InvalidInput(error.to_string()))?;
            let memory = (file.width as u64)
                .checked_mul(file.height as u64)
                .and_then(|value| value.checked_mul(4))
                .and_then(|value| value.checked_mul(file.frames.len() as u64))
                .ok_or(Error::IntegerOverflow("GIF decoded-frame memory"))?;
            (
                file.width,
                file.height,
                file.frames.len() as u32,
                0,
                0,
                memory,
            )
        }
        Format::Ico | Format::Cur => {
            let kind = if FORMAT == Format::Ico {
                legacy_native::IconKind::Icon
            } else {
                legacy_native::IconKind::Cursor
            };
            let members = legacy_native::parse_icon(&source, kind, options.max_members)
                .map_err(|error| Error::InvalidInput(error.to_string()))?;
            let mut peak = 0u64;
            let mut png = 0u32;
            for member in &members {
                if member.png {
                    png += 1;
                }
                peak = peak.max(validate_member(member, options)?);
            }
            let best = legacy_native::select_best(&members)
                .map_err(|error| Error::InvalidInput(error.to_string()))?;
            (
                best.width,
                best.height,
                members.len() as u32,
                png,
                members.len() as u32 - png,
                peak,
            )
        }
    };
    output.write_all(&source)?;
    Ok(Report {
        input_bytes: source.len() as u64,
        output_bytes: source.len() as u64,
        width,
        height,
        frames_or_members: count,
        png_members,
        dib_members,
        peak_working_memory_bytes: source.len() as u64 + decoded_memory,
        strategy: "strict-full-structure-validation",
        backend: "native-portable",
        warnings: Vec::new(),
    })
}

#[doc(hidden)]
pub fn conformance_fixture() -> Vec<u8> {
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
    match FORMAT {
        Format::Gif => legacy_native::encode_gif(&image).expect("fixture GIF"),
        Format::Ico | Format::Cur => {
            let png_image = png_native::Image {
                width: image.width,
                height: image.height,
                source_channels: 4,
                source_bit_depth: 8,
                source_color_type: 6,
                interlaced: false,
                pixels: image
                    .pixels
                    .iter()
                    .map(|pixel| png_native::Pixel16 {
                        r: pixel.r as u16 * 257,
                        g: pixel.g as u16 * 257,
                        b: pixel.b as u16 * 257,
                        a: pixel.a as u16 * 257,
                    })
                    .collect(),
                warnings: Vec::new(),
            };
            let png =
                png_native::encode(&png_image, png_native::Filter::Adaptive).expect("fixture PNG");
            legacy_native::encode_icon(
                &png,
                image.width,
                image.height,
                if FORMAT == Format::Ico {
                    legacy_native::IconKind::Icon
                } else {
                    legacy_native::IconKind::Cursor
                },
                0,
                0,
            )
            .expect("fixture ICO/CUR")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_copy_exact_bytes() {
        let source = conformance_fixture();
        let mut output = Vec::new();
        let report = convert(&mut &source[..], &mut output, &Options::default()).unwrap();
        assert_eq!(output, source);
        assert!(report.frames_or_members > 0);
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

    #[test]
    fn input_limit_is_enforced() {
        let source = conformance_fixture();
        let mut options = Options::default();
        options.max_input_bytes = 8;
        assert!(matches!(
            convert(&mut &source[..], &mut Vec::new(), &options),
            Err(Error::InputTooLarge { .. })
        ));
    }

    #[test]
    fn count_limits_are_enforced() {
        let source = conformance_fixture();
        let mut options = Options::default();
        options.max_frames = 1;
        options.max_members = 1;
        convert(&mut &source[..], &mut Vec::new(), &options).unwrap();
        options.max_frames = 0;
        assert!(matches!(
            convert(&mut &source[..], &mut Vec::new(), &options),
            Err(Error::InvalidOptions(_))
        ));
    }
}
