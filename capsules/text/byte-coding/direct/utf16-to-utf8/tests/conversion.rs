use std::io::{self, Read};

use utf16_to_utf8::{
    convert, ByteOrder, Endianness, Error, InvalidSequencePolicy, Options,
};

fn run(input: &[u8], options: Options) -> (Vec<u8>, utf16_to_utf8::Report) {
    let mut reader = input;
    let mut output = Vec::new();
    let report = convert(&mut reader, &mut output, &options).unwrap();
    (output, report)
}

#[test]
fn defaults_are_runnable_for_bomless_utf16le() {
    let (output, report) = run(&[0x48, 0x00, 0x69, 0x00], Options::default());
    assert_eq!(output, b"Hi");
    assert_eq!(report.detected_endianness, ByteOrder::Little);
    assert!(report.default_endianness_used);
}

#[test]
fn detects_and_consumes_big_endian_bom() {
    let (output, report) = run(&[0xFE, 0xFF, 0x00, 0x41], Options::default());
    assert_eq!(output, b"A");
    assert_eq!(report.detected_endianness, ByteOrder::Big);
    assert!(report.input_bom_consumed);
}

#[test]
fn decodes_supplementary_scalar() {
    let (output, report) = run(&[0x3D, 0xD8, 0x00, 0xDE], Options::default());
    assert_eq!(String::from_utf8(output).unwrap(), "😀");
    assert_eq!(report.decoded_scalar_values, 1);
}

#[test]
fn strict_default_rejects_unpaired_surrogate() {
    let mut input = &[0x00, 0xD8, 0x41, 0x00][..];
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &Options::default()).unwrap_err();
    assert!(matches!(error, Error::UnpairedHighSurrogate { .. }));
}

#[test]
fn replacement_policy_is_explicit_and_reported() {
    let options = Options {
        invalid_sequence: InvalidSequencePolicy::Replace,
        ..Options::default()
    };
    let (output, report) = run(&[0x00, 0xD8, 0x41, 0x00], options);
    assert_eq!(String::from_utf8(output).unwrap(), "�A");
    assert_eq!(report.replacement_count, 1);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn explicit_byte_order_rejects_conflicting_bom() {
    let options = Options {
        input_endianness: Endianness::Little,
        ..Options::default()
    };
    let mut input = &[0xFE, 0xFF, 0x00, 0x41][..];
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &options).unwrap_err();
    assert!(matches!(error, Error::ConflictingBom { .. }));
}

#[test]
fn emits_utf8_bom_when_requested() {
    let options = Options {
        emit_utf8_bom: true,
        ..Options::default()
    };
    let (output, report) = run(&[0x41, 0x00], options);
    assert_eq!(output, [0xEF, 0xBB, 0xBF, 0x41]);
    assert!(report.utf8_bom_emitted);
}

struct OneByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Read for OneByteReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.offset == self.bytes.len() || buffer.is_empty() {
            return Ok(0);
        }
        buffer[0] = self.bytes[self.offset];
        self.offset += 1;
        Ok(1)
    }
}

#[test]
fn preserves_units_across_single_byte_reader_chunks() {
    let bytes = [0xFF, 0xFE, 0x41, 0x00, 0x3D, 0xD8, 0x00, 0xDE];
    let mut reader = OneByteReader {
        bytes: &bytes,
        offset: 0,
    };
    let mut output = Vec::new();
    let report = convert(&mut reader, &mut output, &Options::default()).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "A😀");
    assert_eq!(report.input_bytes, bytes.len() as u64);
}

#[test]
fn rejects_odd_byte_length() {
    let mut input = &[0x41, 0x00, 0x42][..];
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &Options::default()).unwrap_err();
    assert!(matches!(error, Error::OddByteLength { byte_offset: 2 }));
}

