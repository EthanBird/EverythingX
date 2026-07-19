# PNG Wave B native implementation

Date: 2026-07-19  
Status: 20 standalone Capsules implemented and performance-measured.

Normative implementation references: [PNG Third Edition](https://www.w3.org/TR/png-3/),
[RFC 1950 zlib](https://www.rfc-editor.org/rfc/rfc1950.html), and
[RFC 1951 Deflate](https://www.rfc-editor.org/rfc/rfc1951.html).

## Delivered graph

The existing specialized BMP→PNG edge plus nine new direct Capsules complete
all directions between PNG and BMP/TGA/QOI/PPM/PAM. Eleven additional PNG→PNG
Capsules implement validation, canonical normalization, crop, pad, two flips,
three rotations and two alpha-association arithmetic operations.

Every leaf is a complete Rust crate. `src/png_native.rs` is physically copied
into the leaf; it is not imported from EverythingX, a workspace package or a
shared runtime. Deleting `everythingx/` leaves its build, tests, defaults,
errors, report and conformance fixture intact.

## Decoder state machine

The dependency-free decoder recognizes:

- color types 0, 2, 3, 4 and 6;
- each legal 1/2/4/8/16-bit color/depth combination;
- PLTE and tRNS semantics, including packed sub-byte palette indices;
- multiple consecutive IDAT chunks, chunk ordering and unknown-critical rejection;
- CRC-32 and zlib Adler-32;
- stored, fixed-Huffman and dynamic-Huffman Deflate blocks;
- filters None, Sub, Up, Average and Paeth;
- non-interlaced and all seven Adam7 passes.

The decoded proof representation is RGBA16 with checked dimensions and an
explicit inflated-byte limit. Low-depth and 8-bit samples expand exactly onto
the 0..65535 lattice. Transform output remains 16-bit when the source is
16-bit. PNG→8-bit carrier conversions reject 16-bit sources by default and only
scale after `allow_sample_scaling=true`.

## Encoder normalization

Pixel-changing and normalization operators emit deterministic, non-interlaced
RGB or RGBA PNG at 8 or 16 bits. They choose filters adaptively per row, use
stored Deflate, split IDAT at 64 KiB, and regenerate CRC-32 and Adler-32.
`validate-png` is different: it validates the complete source and then copies
the original bytes exactly.

The canonical encoder intentionally strips arbitrary ancillary metadata. That
loss is explicit in the capability record. APNG chunks are not treated as a
single-image conversion promise; animation-aware frame algebra remains a later
Capsule family.

## Tests and evidence

Direct tests cover all encoder filters, RGBA8/RGBA16 round trips, stored/fixed/
dynamic Deflate, Adam7 coordinate reconstruction, 2-bit indexed+tRNS expansion,
CRC/Adler corruption, allocation limits, malformed inputs, operation geometry,
independent output decoding and Adapter default invocation. The batch adds 231
standalone tests and 20 Kernel/Adapter tests.

The controlled baseline covers all 104 production Capsules and 105 capabilities.
PNG Wave B measures 26.616–91.577 MiB/s. The raw cost model, rather than the
display score, is the Planner input. The observed 5.665 memory ratio for PNG
spatial/alpha transforms is a concrete reason to add row-streaming and atomic
spool strategies in a later optimization pass.
