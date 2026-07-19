use std::io::Cursor;

use raw_pcm_to_wav_pcm::{convert, Endianness, Error, IntegerEncoding, Options};

fn data(output: &[u8]) -> &[u8] {
    assert_eq!(&output[..4], b"RIFF");
    assert_eq!(&output[8..12], b"WAVE");
    assert_eq!(&output[36..40], b"data");
    &output[44..44 + u32::from_le_bytes(output[40..44].try_into().unwrap()) as usize]
}

#[test]
fn defaults_wrap_signed_16_little_endian() {
    let source = [0x34, 0x12, 0xcc, 0xed];
    let mut input = Cursor::new(source);
    let mut output = Vec::new();
    let report = convert(&mut input, &mut output, &Options::default()).unwrap();
    assert_eq!(data(&output), source);
    assert_eq!(report.sample_frames, 2);
    assert_eq!(u32::from_le_bytes(output[24..28].try_into().unwrap()), 44_100);
}

#[test]
fn big_endian_samples_are_reversed() {
    let mut options = Options::default();
    options.input_endianness = Endianness::Big;
    let mut input = Cursor::new([0x12, 0x34, 0xab, 0xcd]);
    let mut output = Vec::new();
    convert(&mut input, &mut output, &options).unwrap();
    assert_eq!(data(&output), [0x34, 0x12, 0xcd, 0xab]);
}

#[test]
fn signed_eight_bit_becomes_wave_unsigned() {
    let mut options = Options::default();
    options.bits_per_sample = 8;
    options.input_encoding = IntegerEncoding::Signed;
    let mut input = Cursor::new([0x80, 0x00, 0x7f]);
    let mut output = Vec::new();
    convert(&mut input, &mut output, &options).unwrap();
    assert_eq!(data(&output), [0x00, 0x80, 0xff]);
    assert_eq!(output.len(), 48);
}

#[test]
fn unsigned_sixteen_bit_is_rebiased() {
    let mut options = Options::default();
    options.input_encoding = IntegerEncoding::Unsigned;
    let mut input = Cursor::new([0x00, 0x00, 0xff, 0xff]);
    let mut output = Vec::new();
    convert(&mut input, &mut output, &options).unwrap();
    assert_eq!(data(&output), [0x00, 0x80, 0xff, 0x7f]);
}

#[test]
fn strict_alignment_rejects_partial_frame() {
    let mut input = Cursor::new([1, 2, 3]);
    let error = convert(&mut input, &mut Vec::new(), &Options::default()).unwrap_err();
    assert!(matches!(error, Error::MisalignedInput { .. }));
}

#[test]
fn relaxed_alignment_discards_partial_frame() {
    let mut options = Options::default();
    options.strict_frame_alignment = false;
    let mut input = Cursor::new([1, 2, 3]);
    let mut output = Vec::new();
    let report = convert(&mut input, &mut output, &options).unwrap();
    assert_eq!(data(&output), [1, 2]);
    assert_eq!(report.discarded_trailing_bytes, 1);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn respects_current_input_position() {
    let mut input = Cursor::new([9, 9, 0x34, 0x12]);
    input.set_position(2);
    let mut output = Vec::new();
    convert(&mut input, &mut output, &Options::default()).unwrap();
    assert_eq!(data(&output), [0x34, 0x12]);
}
