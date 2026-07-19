use std::io::{self, Cursor, Read, Seek, SeekFrom};

use wav_pcm_to_aiff::{convert, Error, MetadataPolicy, Options};

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn pcm_format(channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let block = channels * (bits / 8);
    let mut data = Vec::new();
    push_u16(&mut data, 1);
    push_u16(&mut data, channels);
    push_u32(&mut data, rate);
    push_u32(&mut data, rate * block as u32);
    push_u16(&mut data, block);
    push_u16(&mut data, bits);
    data
}

fn extensible_pcm_format(
    channels: u16,
    rate: u32,
    container_bits: u16,
    valid_bits: u16,
) -> Vec<u8> {
    let block = channels * (container_bits / 8);
    let mut data = Vec::new();
    push_u16(&mut data, 0xFFFE);
    push_u16(&mut data, channels);
    push_u32(&mut data, rate);
    push_u32(&mut data, rate * block as u32);
    push_u16(&mut data, block);
    push_u16(&mut data, container_bits);
    push_u16(&mut data, 22);
    push_u16(&mut data, valid_bits);
    push_u32(&mut data, 0);
    data.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
        0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
    ]);
    data
}

fn info_list(entries: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut list = b"INFO".to_vec();
    for (id, value) in entries {
        list.extend_from_slice(id);
        push_u32(&mut list, value.len() as u32);
        list.extend_from_slice(value);
        if value.len() & 1 != 0 {
            list.push(0);
        }
    }
    list
}

fn wave(chunks: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let mut output = b"RIFF\0\0\0\0WAVE".to_vec();
    for (id, data) in chunks {
        output.extend_from_slice(&id);
        push_u32(&mut output, data.len() as u32);
        output.extend_from_slice(&data);
        if data.len() & 1 != 0 {
            output.push(0);
        }
    }
    let size = output.len() as u32 - 8;
    output[4..8].copy_from_slice(&size.to_le_bytes());
    output
}

#[derive(Debug)]
struct Aiff {
    common: Vec<u8>,
    metadata: Vec<([u8; 4], Vec<u8>)>,
    sound: Vec<u8>,
}

fn parse_aiff(bytes: &[u8]) -> Aiff {
    assert_eq!(&bytes[0..4], b"FORM");
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize + 8, bytes.len());
    assert_eq!(&bytes[8..12], b"AIFF");
    let mut common = None;
    let mut metadata = Vec::new();
    let mut sound = None;
    let mut position = 12;
    while position + 8 <= bytes.len() {
        let id: [u8; 4] = bytes[position..position + 4].try_into().unwrap();
        let size = u32::from_be_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        let data = &bytes[position + 8..position + 8 + size];
        match &id {
            b"COMM" => common = Some(data.to_vec()),
            b"SSND" => {
                assert_eq!(&data[0..8], &[0; 8]);
                sound = Some(data[8..].to_vec());
            }
            _ => metadata.push((id, data.to_vec())),
        }
        position += 8 + size + (size & 1);
    }
    assert_eq!(position, bytes.len());
    Aiff {
        common: common.unwrap(),
        metadata,
        sound: sound.unwrap(),
    }
}

fn run(input: Vec<u8>, options: Options) -> (Vec<u8>, wav_pcm_to_aiff::Report) {
    let mut input = Cursor::new(input);
    let mut output = Vec::new();
    let report = convert(&mut input, &mut output, &options).unwrap();
    (output, report)
}

#[test]
fn defaults_convert_unsigned_eight_bit_pcm_to_signed_aiff() {
    let source = wave(vec![
        (*b"fmt ", pcm_format(1, 44_100, 8)),
        (*b"data", vec![0, 128, 255]),
    ]);
    let (output, report) = run(source, Options::default());
    let aiff = parse_aiff(&output);
    assert_eq!(aiff.sound, [0x80, 0x00, 0x7F]);
    assert_eq!(u16::from_be_bytes(aiff.common[0..2].try_into().unwrap()), 1);
    assert_eq!(u32::from_be_bytes(aiff.common[2..6].try_into().unwrap()), 3);
    assert_eq!(u16::from_be_bytes(aiff.common[6..8].try_into().unwrap()), 8);
    assert_eq!(&aiff.common[8..18], &[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
    assert_eq!(report.sample_frames, 3);
    assert_eq!(report.output_audio_bytes, 3);
}

#[test]
fn swaps_sixteen_bit_stereo_samples_without_changing_channel_order() {
    let source = wave(vec![
        (*b"fmt ", pcm_format(2, 48_000, 16)),
        (*b"data", vec![0x34, 0x12, 0xCC, 0xED]),
    ]);
    let (output, report) = run(source, Options::default());
    let aiff = parse_aiff(&output);
    assert_eq!(aiff.sound, [0x12, 0x34, 0xED, 0xCC]);
    assert_eq!(report.channels, 2);
    assert_eq!(report.sample_frames, 1);
}

#[test]
fn swaps_twenty_four_and_thirty_two_bit_samples() {
    for (bits, source, expected) in [
        (24, vec![3, 2, 1], vec![1, 2, 3]),
        (32, vec![4, 3, 2, 1], vec![1, 2, 3, 4]),
    ] {
        let wav = wave(vec![
            (*b"fmt ", pcm_format(1, 96_000, bits)),
            (*b"data", source),
        ]);
        let (output, _) = run(wav, Options::default());
        assert_eq!(parse_aiff(&output).sound, expected);
    }
}

#[test]
fn extensible_valid_bits_shrink_left_aligned_container_samples() {
    let source = wave(vec![
        (*b"fmt ", extensible_pcm_format(1, 48_000, 32, 24)),
        (*b"data", vec![0x00, 0x56, 0x34, 0x12]),
    ]);
    let (output, report) = run(source, Options::default());
    let aiff = parse_aiff(&output);
    assert_eq!(aiff.sound, [0x12, 0x34, 0x56]);
    assert_eq!(u16::from_be_bytes(aiff.common[6..8].try_into().unwrap()), 24);
    assert!(report.wave_format_extensible);
    assert_eq!(report.source_audio_bytes, 4);
    assert_eq!(report.output_audio_bytes, 3);
}

#[test]
fn maps_common_info_metadata_and_discards_unknown_info_fields() {
    let list = info_list(&[
        (*b"INAM", b"Example\0"),
        (*b"IART", b"Artist"),
        (*b"ICMT", b"Note"),
        (*b"ICOP", b"Copyright"),
        (*b"ISFT", b"Ignored"),
    ]);
    let source = wave(vec![
        (*b"fmt ", pcm_format(1, 8_000, 8)),
        (*b"LIST", list),
        (*b"data", vec![128]),
    ]);
    let (output, report) = run(source, Options::default());
    let aiff = parse_aiff(&output);
    assert_eq!(
        aiff.metadata,
        vec![
            (*b"NAME", b"Example".to_vec()),
            (*b"AUTH", b"Artist".to_vec()),
            (*b"ANNO", b"Note".to_vec()),
            (*b"(c) ", b"Copyright".to_vec()),
        ]
    );
    assert_eq!(report.metadata_chunks_found, 4);
    assert_eq!(report.metadata_chunks_preserved, 4);
}

#[test]
fn metadata_discard_policy_is_runnable_and_reported() {
    let source = wave(vec![
        (*b"fmt ", pcm_format(1, 8_000, 8)),
        (*b"LIST", info_list(&[(*b"INAM", b"Name")])),
        (*b"data", vec![128]),
    ]);
    let options = Options {
        metadata: MetadataPolicy::Discard,
        ..Options::default()
    };
    let (output, report) = run(source, options);
    assert!(parse_aiff(&output).metadata.is_empty());
    assert_eq!(report.metadata_chunks_found, 1);
    assert_eq!(report.metadata_chunks_preserved, 0);
}

#[test]
fn scans_data_before_format_and_concatenates_multiple_aligned_data_chunks() {
    let source = wave(vec![
        (*b"data", vec![1, 0]),
        (*b"JUNK", vec![9, 8, 7]),
        (*b"fmt ", pcm_format(1, 44_100, 16)),
        (*b"data", vec![2, 0]),
    ]);
    let (output, report) = run(source, Options::default());
    assert_eq!(parse_aiff(&output).sound, [0, 1, 0, 2]);
    assert_eq!(report.source_data_chunks, 2);
    assert_eq!(report.sample_frames, 2);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn strict_defaults_reject_inconsistent_byte_rate() {
    let mut fmt = pcm_format(1, 44_100, 16);
    fmt[8..12].copy_from_slice(&1_u32.to_le_bytes());
    let source = wave(vec![(*b"fmt ", fmt), (*b"data", vec![0, 0])]);
    let mut input = Cursor::new(source);
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &Options::default()).unwrap_err();
    assert!(matches!(error, Error::InvalidByteRate { .. }));
}

#[test]
fn relaxed_header_mode_normalizes_inconsistent_rates() {
    let mut fmt = pcm_format(1, 44_100, 16);
    fmt[8..12].copy_from_slice(&1_u32.to_le_bytes());
    let source = wave(vec![(*b"fmt ", fmt), (*b"data", vec![0, 0])]);
    let options = Options {
        strict_header_consistency: false,
        ..Options::default()
    };
    let (_, report) = run(source, options);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn rejects_float_and_partial_sample_frames() {
    let mut float_fmt = pcm_format(1, 44_100, 32);
    float_fmt[0..2].copy_from_slice(&3_u16.to_le_bytes());
    let source = wave(vec![(*b"fmt ", float_fmt), (*b"data", vec![0; 4])]);
    let mut input = Cursor::new(source);
    let mut output = Vec::new();
    assert!(matches!(
        convert(&mut input, &mut output, &Options::default()).unwrap_err(),
        Error::UnsupportedFormatTag(3)
    ));

    let source = wave(vec![
        (*b"fmt ", pcm_format(2, 44_100, 16)),
        (*b"data", vec![0, 0]),
    ]);
    let mut input = Cursor::new(source);
    let error = convert(&mut input, &mut output, &Options::default()).unwrap_err();
    assert!(matches!(error, Error::PartialSampleFrame { .. }));
}

#[test]
fn rejects_rf64_as_an_explicit_classic_aiff_boundary() {
    let mut source = wave(vec![
        (*b"fmt ", pcm_format(1, 44_100, 16)),
        (*b"data", vec![0, 0]),
    ]);
    source[0..4].copy_from_slice(b"RF64");
    let mut input = Cursor::new(source);
    let mut output = Vec::new();
    assert!(matches!(
        convert(&mut input, &mut output, &Options::default()).unwrap_err(),
        Error::UnsupportedRf64
    ));
}

struct OneByteSeekReader {
    inner: Cursor<Vec<u8>>,
}

impl Read for OneByteSeekReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.inner.read(&mut buffer[..1])
    }
}

impl Seek for OneByteSeekReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn supports_fragmented_readers_across_headers_and_samples() {
    let source = wave(vec![
        (*b"fmt ", pcm_format(1, 44_100, 16)),
        (*b"data", vec![0x34, 0x12, 0x78, 0x56]),
    ]);
    let mut input = OneByteSeekReader {
        inner: Cursor::new(source),
    };
    let mut output = Vec::new();
    convert(&mut input, &mut output, &Options::default()).unwrap();
    assert_eq!(parse_aiff(&output).sound, [0x12, 0x34, 0x56, 0x78]);
}

