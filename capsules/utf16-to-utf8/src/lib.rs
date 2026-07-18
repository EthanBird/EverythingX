#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Write};

const UTF8_BOM: &[u8; 3] = b"\xEF\xBB\xBF";
const REPLACEMENT: char = '\u{FFFD}';
const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;
const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Auto,
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSequencePolicy {
    Error,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub input_endianness: Endianness,
    pub default_endianness: ByteOrder,
    pub invalid_sequence: InvalidSequencePolicy,
    pub emit_utf8_bom: bool,
    pub buffer_size: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input_endianness: Endianness::Auto,
            default_endianness: ByteOrder::Little,
            invalid_sequence: InvalidSequencePolicy::Error,
            emit_utf8_bom: false,
            buffer_size: DEFAULT_BUFFER_SIZE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub decoded_scalar_values: u64,
    pub replacement_count: u64,
    pub detected_endianness: ByteOrder,
    pub input_bom_consumed: bool,
    pub default_endianness_used: bool,
    pub utf8_bom_emitted: bool,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    OddByteLength { byte_offset: u64 },
    ConflictingBom { configured: ByteOrder, bom: ByteOrder },
    UnpairedHighSurrogate { code_unit_index: u64, value: u16 },
    UnexpectedLowSurrogate { code_unit_index: u64, value: u16 },
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => write!(formatter, "invalid options: {message}"),
            Self::OddByteLength { byte_offset } => {
                write!(formatter, "UTF-16 input ends with an unmatched byte at offset {byte_offset}")
            }
            Self::ConflictingBom { configured, bom } => {
                write!(formatter, "configured {configured:?}-endian input conflicts with {bom:?}-endian BOM")
            }
            Self::UnpairedHighSurrogate { code_unit_index, value } => {
                write!(formatter, "unpaired high surrogate 0x{value:04X} at code-unit index {code_unit_index}")
            }
            Self::UnexpectedLowSurrogate { code_unit_index, value } => {
                write!(formatter, "unexpected low surrogate 0x{value:04X} at code-unit index {code_unit_index}")
            }
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

#[derive(Debug, Default)]
struct DecodeState {
    pending_high: Option<(u16, u64)>,
    decoded_scalar_values: u64,
    replacement_count: u64,
    output_bytes: u64,
}

fn write_char<W: Write>(output: &mut W, value: char, state: &mut DecodeState) -> Result<(), Error> {
    let mut encoded = [0_u8; 4];
    let bytes = value.encode_utf8(&mut encoded).as_bytes();
    output.write_all(bytes)?;
    state.output_bytes += bytes.len() as u64;
    state.decoded_scalar_values += 1;
    Ok(())
}

fn write_replacement<W: Write>(output: &mut W, state: &mut DecodeState) -> Result<(), Error> {
    write_char(output, REPLACEMENT, state)?;
    state.replacement_count += 1;
    Ok(())
}

fn process_unit<W: Write>(
    unit: u16,
    unit_index: u64,
    policy: InvalidSequencePolicy,
    output: &mut W,
    state: &mut DecodeState,
) -> Result<(), Error> {
    if let Some((high, high_index)) = state.pending_high.take() {
        if (0xDC00..=0xDFFF).contains(&unit) {
            let scalar = 0x10000
                + (((high as u32) - 0xD800) << 10)
                + ((unit as u32) - 0xDC00);
            let value = char::from_u32(scalar).expect("valid surrogate pair always forms a scalar");
            return write_char(output, value, state);
        }
        match policy {
            InvalidSequencePolicy::Error => {
                return Err(Error::UnpairedHighSurrogate {
                    code_unit_index: high_index,
                    value: high,
                });
            }
            InvalidSequencePolicy::Replace => write_replacement(output, state)?,
        }
    }

    match unit {
        0xD800..=0xDBFF => {
            state.pending_high = Some((unit, unit_index));
            Ok(())
        }
        0xDC00..=0xDFFF => match policy {
            InvalidSequencePolicy::Error => Err(Error::UnexpectedLowSurrogate {
                code_unit_index: unit_index,
                value: unit,
            }),
            InvalidSequencePolicy::Replace => write_replacement(output, state),
        },
        _ => write_char(
            output,
            char::from_u32(unit as u32).expect("non-surrogate u16 is a Unicode scalar"),
            state,
        ),
    }
}

fn decode_unit(bytes: [u8; 2], order: ByteOrder) -> u16 {
    match order {
        ByteOrder::Little => u16::from_le_bytes(bytes),
        ByteOrder::Big => u16::from_be_bytes(bytes),
    }
}

fn read_initial_pair<R: Read>(input: &mut R) -> Result<(Option<[u8; 2]>, u64), Error> {
    let mut pair = [0_u8; 2];
    let mut count = 0;
    while count < 2 {
        let read = input.read(&mut pair[count..])?;
        if read == 0 {
            break;
        }
        count += read;
    }
    match count {
        0 => Ok((None, 0)),
        1 => Err(Error::OddByteLength { byte_offset: 0 }),
        2 => Ok((Some(pair), 2)),
        _ => unreachable!(),
    }
}

pub fn convert<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    if options.buffer_size < 2 {
        return Err(Error::InvalidOptions("buffer_size must be at least 2 bytes"));
    }
    if options.buffer_size > MAX_BUFFER_SIZE {
        return Err(Error::InvalidOptions("buffer_size exceeds 16 MiB"));
    }

    let (first_pair, mut input_bytes) = read_initial_pair(input)?;
    let first_pair = first_pair.unwrap_or([0, 0]);
    let bom_order = match first_pair {
        [0xFF, 0xFE] => Some(ByteOrder::Little),
        [0xFE, 0xFF] => Some(ByteOrder::Big),
        _ => None,
    };
    let configured_order = match options.input_endianness {
        Endianness::Auto => None,
        Endianness::Little => Some(ByteOrder::Little),
        Endianness::Big => Some(ByteOrder::Big),
    };
    if let (Some(configured), Some(bom)) = (configured_order, bom_order) {
        if configured != bom {
            return Err(Error::ConflictingBom { configured, bom });
        }
    }
    let order = configured_order.or(bom_order).unwrap_or(options.default_endianness);
    let input_bom_consumed = first_pair != [0, 0] && bom_order.is_some();
    let default_endianness_used = configured_order.is_none() && bom_order.is_none();
    let mut state = DecodeState::default();
    if options.emit_utf8_bom {
        output.write_all(UTF8_BOM)?;
        state.output_bytes += UTF8_BOM.len() as u64;
    }

    let mut unit_index = 0_u64;
    if input_bom_consumed {
        unit_index = 1;
    } else if input_bytes == 2 {
        process_unit(
            decode_unit(first_pair, order),
            unit_index,
            options.invalid_sequence,
            output,
            &mut state,
        )?;
        unit_index += 1;
    }

    let mut buffer = vec![0_u8; options.buffer_size];
    let mut carry: Option<u8> = None;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        input_bytes += count as u64;
        let mut offset = 0;
        if let Some(first) = carry.take() {
            process_unit(
                decode_unit([first, buffer[0]], order),
                unit_index,
                options.invalid_sequence,
                output,
                &mut state,
            )?;
            unit_index += 1;
            offset = 1;
        }
        while offset + 1 < count {
            process_unit(
                decode_unit([buffer[offset], buffer[offset + 1]], order),
                unit_index,
                options.invalid_sequence,
                output,
                &mut state,
            )?;
            unit_index += 1;
            offset += 2;
        }
        if offset < count {
            carry = Some(buffer[offset]);
        }
    }

    if carry.is_some() {
        return Err(Error::OddByteLength {
            byte_offset: input_bytes - 1,
        });
    }
    if let Some((high, high_index)) = state.pending_high.take() {
        match options.invalid_sequence {
            InvalidSequencePolicy::Error => {
                return Err(Error::UnpairedHighSurrogate {
                    code_unit_index: high_index,
                    value: high,
                });
            }
            InvalidSequencePolicy::Replace => write_replacement(output, &mut state)?,
        }
    }

    let warnings = if state.replacement_count == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{} malformed UTF-16 sequence(s) replaced with U+FFFD",
            state.replacement_count
        )]
    };
    Ok(Report {
        input_bytes,
        output_bytes: state.output_bytes,
        decoded_scalar_values: state.decoded_scalar_values,
        replacement_count: state.replacement_count,
        detected_endianness: order,
        input_bom_consumed,
        default_endianness_used,
        utf8_bom_emitted: options.emit_utf8_bom,
        strategy: match options.invalid_sequence {
            InvalidSequencePolicy::Error => "strict",
            InvalidSequencePolicy::Replace => "replace-invalid",
        },
        backend: "native-portable",
        warnings,
    })
}
