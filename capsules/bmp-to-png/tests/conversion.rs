use std::io::Cursor;

use bmp_to_png::{
    convert, CompressionStrategy, Error, FilterStrategy, Options, UnmarkedAlpha,
};

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn bmp_header(
    width: i32,
    height: i32,
    bits_per_pixel: u16,
    compression: u32,
    image_size: u32,
    palette_entries: u32,
    extra_header_bytes: u32,
) -> Vec<u8> {
    let pixel_offset = 14 + 40 + extra_header_bytes + palette_entries * 4;
    let file_size = pixel_offset + image_size;
    let mut bmp = Vec::new();
    bmp.extend_from_slice(b"BM");
    push_u32(&mut bmp, file_size);
    push_u32(&mut bmp, 0);
    push_u32(&mut bmp, pixel_offset);
    push_u32(&mut bmp, 40);
    push_i32(&mut bmp, width);
    push_i32(&mut bmp, height);
    push_u16(&mut bmp, 1);
    push_u16(&mut bmp, bits_per_pixel);
    push_u32(&mut bmp, compression);
    push_u32(&mut bmp, image_size);
    push_i32(&mut bmp, 2_835);
    push_i32(&mut bmp, 2_835);
    push_u32(&mut bmp, palette_entries);
    push_u32(&mut bmp, 0);
    bmp
}

fn bmp_24_bottom_up() -> Vec<u8> {
    let mut bmp = bmp_header(2, 2, 24, 0, 16, 0, 0);
    // Bottom row: blue, white. Each six-byte row has two padding bytes.
    bmp.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
    // Top row: red, green.
    bmp.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    bmp
}

fn bmp_24_top_down() -> Vec<u8> {
    let mut bmp = bmp_header(2, -2, 24, 0, 16, 0, 0);
    bmp.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    bmp.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
    bmp
}

fn bmp_4_indexed() -> Vec<u8> {
    let mut bmp = bmp_header(3, 1, 4, 0, 4, 4, 0);
    // Palette is B,G,R,reserved.
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&[0, 0, 255, 0]);
    bmp.extend_from_slice(&[0, 255, 0, 0]);
    bmp.extend_from_slice(&[255, 0, 0, 0]);
    // Indexes 1,2,3 followed by row padding.
    bmp.extend_from_slice(&[0x12, 0x30, 0, 0]);
    bmp
}

fn bmp_32_bi_rgb() -> Vec<u8> {
    let mut bmp = bmp_header(1, 1, 32, 0, 4, 0, 0);
    bmp.extend_from_slice(&[10, 20, 30, 40]);
    bmp
}

fn bmp_16_bitfields() -> Vec<u8> {
    let mut bmp = bmp_header(3, 1, 16, 3, 8, 0, 12);
    // RGB 5:6:5 masks stored after the 40-byte DIB header.
    push_u32(&mut bmp, 0xF800);
    push_u32(&mut bmp, 0x07E0);
    push_u32(&mut bmp, 0x001F);
    push_u16(&mut bmp, 0xF800);
    push_u16(&mut bmp, 0x07E0);
    push_u16(&mut bmp, 0x001F);
    push_u16(&mut bmp, 0);
    bmp
}

fn bmp_rle8() -> Vec<u8> {
    let commands = [
        2, 3, // bottom row: blue, blue
        0, 0, // EOL
        0, 2, 1, 2, // absolute count two is reserved as DELTA, so encode separately below
    ];
    let actual = [
        2, 3, 0, 0, // bottom row
        1, 1, 1, 2, 0, 0, // top row: red, green, EOL
        0, 1, // EOB
    ];
    let _ = commands;
    let mut bmp = bmp_header(2, 2, 8, 1, actual.len() as u32, 4, 0);
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&[0, 0, 255, 0]);
    bmp.extend_from_slice(&[0, 255, 0, 0]);
    bmp.extend_from_slice(&[255, 0, 0, 0]);
    bmp.extend_from_slice(&actual);
    bmp
}

fn fixed_code(symbol: u16) -> (u32, u8) {
    match symbol {
        0..=143 => (0x30 + symbol as u32, 8),
        144..=255 => (0x190 + (symbol as u32 - 144), 9),
        256..=279 => (symbol as u32 - 256, 7),
        280..=287 => (0xC0 + (symbol as u32 - 280), 8),
        _ => unreachable!(),
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    byte: usize,
    bit: u8,
}

impl BitReader<'_> {
    fn one(&mut self) -> u32 {
        let result = ((self.bytes[self.byte] >> self.bit) & 1) as u32;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.byte += 1;
        }
        result
    }

    fn bits(&mut self, count: u8) -> u32 {
        let mut result = 0;
        for shift in 0..count {
            result |= self.one() << shift;
        }
        result
    }

    fn align(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.byte += 1;
        }
    }

    fn fixed_symbol(&mut self) -> u16 {
        let mut transmitted = 0_u32;
        for length in 1..=9 {
            transmitted = (transmitted << 1) | self.one();
            for symbol in 0..=287 {
                let (code, bits) = fixed_code(symbol);
                if bits == length && code == transmitted {
                    return symbol;
                }
            }
        }
        panic!("invalid fixed Huffman code")
    }

    fn fixed_distance_symbol(&mut self) -> usize {
        let mut symbol = 0_usize;
        for _ in 0..5 {
            symbol = (symbol << 1) | self.one() as usize;
        }
        symbol
    }
}

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51,
    59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4,
    4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385,
    513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9,
    10, 10, 11, 11, 12, 12, 13, 13,
];

fn adler32(bytes: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in bytes {
        a = (a + *byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn inflate_zlib(stream: &[u8]) -> Vec<u8> {
    assert_eq!(((stream[0] as u16) * 256 + stream[1] as u16) % 31, 0);
    let expected_adler = u32::from_be_bytes(stream[stream.len() - 4..].try_into().unwrap());
    let mut reader = BitReader {
        bytes: &stream[2..stream.len() - 4],
        byte: 0,
        bit: 0,
    };
    let mut output = Vec::new();
    loop {
        let final_block = reader.bits(1) != 0;
        let block_type = reader.bits(2);
        match block_type {
            0 => {
                reader.align();
                let bytes = reader.bytes;
                let offset = reader.byte;
                let length = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                let inverse = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
                assert_eq!(inverse, !(length as u16));
                output.extend_from_slice(&bytes[offset + 4..offset + 4 + length]);
                reader.byte += 4 + length;
            }
            1 => loop {
                let symbol = reader.fixed_symbol();
                match symbol {
                    0..=255 => output.push(symbol as u8),
                    256 => break,
                    257..=285 => {
                        let index = symbol as usize - 257;
                        let length = LENGTH_BASE[index] + reader.bits(LENGTH_EXTRA[index]) as usize;
                        let distance_symbol = reader.fixed_distance_symbol();
                        let distance = DISTANCE_BASE[distance_symbol]
                            + reader.bits(DISTANCE_EXTRA[distance_symbol]) as usize;
                        for _ in 0..length {
                            let value = output[output.len() - distance];
                            output.push(value);
                        }
                    }
                    _ => panic!("reserved fixed symbol"),
                }
            },
            _ => panic!("test decoder only accepts stored or fixed blocks"),
        }
        if final_block {
            break;
        }
    }
    assert_eq!(adler32(&output), expected_adler);
    output
}

fn paeth(left: u8, above: u8, upper_left: u8) -> u8 {
    let estimate = left as i32 + above as i32 - upper_left as i32;
    let dl = (estimate - left as i32).abs();
    let da = (estimate - above as i32).abs();
    let dd = (estimate - upper_left as i32).abs();
    if dl <= da && dl <= dd { left } else if da <= dd { above } else { upper_left }
}

fn decode_png(png: &[u8]) -> (u32, u32, usize, Vec<u8>, usize) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1A\n");
    let mut offset = 8;
    let mut width = 0;
    let mut height = 0;
    let mut channels = 0;
    let mut idat = Vec::new();
    let mut idat_chunks = 0;
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data = &png[offset + 8..offset + 8 + length];
        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                assert_eq!(data[8], 8);
                channels = if data[9] == 6 { 4 } else { 3 };
            }
            b"IDAT" => {
                idat.extend_from_slice(data);
                idat_chunks += 1;
            }
            b"IEND" => break,
            _ => {}
        }
        offset += 12 + length;
    }
    let filtered = inflate_zlib(&idat);
    let row_length = width as usize * channels;
    assert_eq!(filtered.len(), (row_length + 1) * height as usize);
    let mut pixels = Vec::with_capacity(row_length * height as usize);
    let mut previous = vec![0_u8; row_length];
    for row in filtered.chunks_exact(row_length + 1) {
        let filter = row[0];
        let mut decoded = vec![0_u8; row_length];
        for index in 0..row_length {
            let left = if index >= channels { decoded[index - channels] } else { 0 };
            let above = previous[index];
            let upper_left = if index >= channels { previous[index - channels] } else { 0 };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => above,
                3 => ((left as u16 + above as u16) / 2) as u8,
                4 => paeth(left, above, upper_left),
                _ => panic!("unknown PNG filter"),
            };
            decoded[index] = row[index + 1].wrapping_add(predictor);
        }
        pixels.extend_from_slice(&decoded);
        previous = decoded;
    }
    (width, height, channels, pixels, idat_chunks)
}

fn run(bytes: Vec<u8>, options: Options) -> (Vec<u8>, bmp_to_png::Report) {
    let mut input = Cursor::new(bytes);
    let mut output = Vec::new();
    let report = convert(&mut input, &mut output, &options).unwrap();
    (output, report)
}

#[test]
fn defaults_convert_bottom_up_bgr_to_top_down_rgb() {
    let (png, report) = run(bmp_24_bottom_up(), Options::default());
    let (width, height, channels, pixels, _) = decode_png(&png);
    assert_eq!((width, height, channels), (2, 2, 3));
    assert_eq!(
        pixels,
        [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
    );
    assert_eq!(report.compression, "fixed-rle");
    assert!(!report.source_top_down);
}

#[test]
fn store_mode_and_none_filter_are_valid_png() {
    let options = Options {
        compression: CompressionStrategy::Store,
        filter: FilterStrategy::None,
        ..Options::default()
    };
    let (png, report) = run(bmp_24_top_down(), options);
    let (_, _, _, pixels, _) = decode_png(&png);
    assert_eq!(pixels[0..6], [255, 0, 0, 0, 255, 0]);
    assert!(report.source_top_down);
}

#[test]
fn decodes_four_bit_palette() {
    let (png, report) = run(bmp_4_indexed(), Options::default());
    let (_, _, channels, pixels, _) = decode_png(&png);
    assert_eq!(channels, 3);
    assert_eq!(pixels, [255, 0, 0, 0, 255, 0, 0, 0, 255]);
    assert_eq!(report.palette_entries, 4);
}

#[test]
fn unmarked_alpha_default_is_opaque_but_can_be_preserved() {
    let (opaque_png, opaque_report) = run(bmp_32_bi_rgb(), Options::default());
    let (_, _, channels, pixels, _) = decode_png(&opaque_png);
    assert_eq!(channels, 3);
    assert_eq!(pixels, [30, 20, 10]);
    assert!(!opaque_report.alpha_preserved);

    let options = Options {
        unmarked_alpha: UnmarkedAlpha::Preserve,
        ..Options::default()
    };
    let (alpha_png, alpha_report) = run(bmp_32_bi_rgb(), options);
    let (_, _, channels, pixels, _) = decode_png(&alpha_png);
    assert_eq!(channels, 4);
    assert_eq!(pixels, [30, 20, 10, 40]);
    assert!(alpha_report.alpha_preserved);
}

#[test]
fn scales_rgb565_bitfields_to_eight_bit_channels() {
    let (png, _) = run(bmp_16_bitfields(), Options::default());
    let (_, _, _, pixels, _) = decode_png(&png);
    assert_eq!(pixels, [255, 0, 0, 0, 255, 0, 0, 0, 255]);
}

#[test]
fn decodes_rle8_and_restores_top_down_order() {
    let (png, report) = run(bmp_rle8(), Options::default());
    let (_, _, _, pixels, _) = decode_png(&png);
    assert_eq!(pixels, [255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255]);
    assert_eq!(report.source_compression, "BI_RLE8");
}

#[test]
fn small_idat_chunks_remain_one_logical_zlib_stream() {
    let options = Options {
        idat_chunk_size: 7,
        ..Options::default()
    };
    let (png, _) = run(bmp_24_bottom_up(), options);
    let (_, _, _, pixels, chunks) = decode_png(&png);
    assert!(chunks > 1);
    assert_eq!(pixels.len(), 12);
}

#[test]
fn rejects_pixel_counts_above_budget() {
    let options = Options {
        max_pixels: 3,
        ..Options::default()
    };
    let mut input = Cursor::new(bmp_24_bottom_up());
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &options).unwrap_err();
    assert!(matches!(error, Error::PixelLimitExceeded { .. }));
}

#[test]
fn rejects_invalid_signature() {
    let mut bytes = bmp_24_bottom_up();
    bytes[0] = b'Z';
    let mut input = Cursor::new(bytes);
    let mut output = Vec::new();
    let error = convert(&mut input, &mut output, &Options::default()).unwrap_err();
    assert!(matches!(error, Error::InvalidSignature));
}

