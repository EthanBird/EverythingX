# bmp-to-png

A standalone, zero-dependency Rust library that converts supported Windows BMP
pixel arrays into valid PNG byte streams. It has no EverythingX dependency and
continues to build after the optional `everythingx/` directory is deleted.

## Runnable default

```rust
use std::io::Cursor;

let mut input = Cursor::new(std::fs::read("input.bmp")?);
let mut output = std::fs::File::create("output.png")?;
let report = bmp_to_png::convert(
    &mut input,
    &mut output,
    &bmp_to_png::Options::default(),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Defaults use adaptive PNG row filtering, the native fixed-Huffman/RLE Deflate
encoder, opaque interpretation of undeclared BMP alpha bytes, 64 KiB IDAT
chunks, and a 100-million-pixel input limit.

## Supported BMP input

- Windows DIB headers of 40 through 4096 bytes;
- 1-, 4-, and 8-bit indexed BI_RGB;
- 16-bit BI_RGB (5:5:5) and 16/32-bit BI_BITFIELDS;
- 24-bit BGR and 32-bit BGRX/BGRA-style BI_RGB;
- bottom-up and top-down uncompressed rows;
- RLE4 and RLE8 command streams;
- explicit preservation of undeclared alpha through an option.

PNG output is RGB8 or RGBA8. Pixel color values and spatial order are
preserved for accepted inputs. Palette structure, row order, padding, and BMP
container metadata are normalized; the report makes those boundaries visible.

## Deliberate limits in 0.1

- OS/2 bitmap headers, JPEG/PNG-compressed BMP payloads and color profiles are
  rejected or not migrated;
- the fixed-Huffman encoder currently searches distance-one runs only;
- metadata such as resolution and application fields is not emitted yet;
- RLE input requires one byte per decoded palette index in memory.

`CompressionStrategy::Store` is also available when minimum CPU work is more
important than PNG size.

