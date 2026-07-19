# utf16-to-utf8

An independent, zero-dependency, streaming Rust library that converts UTF-16 byte streams to UTF-8. It has no EverythingX dependency and remains buildable after deleting `everythingx/`.

## Runnable defaults

`Options::default()` is a complete conversion policy:

- detect byte order from BOM;
- use little-endian when no BOM is present;
- consume a valid UTF-16 BOM;
- reject malformed surrogate sequences;
- do not emit a UTF-8 BOM;
- use a 64 KiB streaming buffer.

These defaults are deterministic and intentionally strict. No configuration is required for a valid UTF-16LE input without a BOM.

```rust
use utf16_to_utf8::{convert, Options};

let mut input = &[0x48, 0x00, 0x69, 0x00][..];
let mut output = Vec::new();
let report = convert(&mut input, &mut output, &Options::default())?;
assert_eq!(output, b"Hi");
# Ok::<(), utf16_to_utf8::Error>(())
```

The library handles BOM detection, odd reader chunk boundaries, surrogate pairs, malformed-sequence policy, optional UTF-8 BOM output and bounded buffer allocation. The `Report` records detected byte order, BOM use, scalar/replacement counts and byte counts.

## EverythingX integration

`everythingx/` contains an optional Adapter. It maps protocol options into this library's native `Options`; the core public API contains no EverythingX types.

