use std::io::Cursor;
use wav_pcm_to_raw_pcm::{convert, Endianness, Error, IntegerEncoding, Options};

fn wave(bits: u16, channels: u16, rate: u32, chunks: &[&[u8]]) -> Vec<u8> {
    let bytes = u32::from(bits / 8);
    let align = u32::from(channels) * bytes;
    let mut body = b"WAVEfmt ".to_vec();
    body.extend_from_slice(&16_u32.to_le_bytes());
    body.extend_from_slice(&1_u16.to_le_bytes());
    body.extend_from_slice(&channels.to_le_bytes());
    body.extend_from_slice(&rate.to_le_bytes());
    body.extend_from_slice(&(rate * align).to_le_bytes());
    body.extend_from_slice(&(align as u16).to_le_bytes());
    body.extend_from_slice(&bits.to_le_bytes());
    for chunk in chunks {
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        body.extend_from_slice(chunk);
        if chunk.len() & 1 != 0 { body.push(0); }
    }
    let mut result = b"RIFF".to_vec();
    result.extend_from_slice(&(body.len() as u32).to_le_bytes());
    result.extend_from_slice(&body);
    result
}

#[test]
fn defaults_extract_signed_sixteen_bit() {
    let bytes = wave(16, 1, 44_100, &[&[0x34, 0x12, 0xcc, 0xed]]);
    let report = convert(&mut Cursor::new(bytes), &mut Vec::new(), &Options::default()).unwrap();
    assert_eq!(report.sample_frames, 2);
}

#[test]
fn eight_bit_wave_unsigned_becomes_signed_raw() {
    let bytes = wave(8, 1, 8_000, &[&[0x00, 0x80, 0xff]]);
    let mut output = Vec::new();
    convert(&mut Cursor::new(bytes), &mut output, &Options::default()).unwrap();
    assert_eq!(output, [0x80, 0x00, 0x7f]);
}

#[test]
fn can_emit_big_endian_unsigned_pcm() {
    let bytes = wave(16, 1, 8_000, &[&[0x00, 0x80, 0xff, 0x7f]]);
    let mut options = Options::default();
    options.output_endianness = Endianness::Big;
    options.output_encoding = IntegerEncoding::Unsigned;
    let mut output = Vec::new();
    convert(&mut Cursor::new(bytes), &mut output, &options).unwrap();
    assert_eq!(output, [0x00, 0x00, 0xff, 0xff]);
}

#[test]
fn concatenates_multiple_data_chunks() {
    let bytes = wave(16, 1, 8_000, &[&[1, 2], &[3, 4]]);
    let mut output = Vec::new();
    let report = convert(&mut Cursor::new(bytes), &mut output, &Options::default()).unwrap();
    assert_eq!(output, [1, 2, 3, 4]);
    assert_eq!(report.source_data_chunks, 2);
}

#[test]
fn rejects_non_wave() {
    let error = convert(&mut Cursor::new(b"not a wave"), &mut Vec::new(), &Options::default()).unwrap_err();
    assert!(matches!(error, Error::Io(_) | Error::InvalidWave(_)));
}
