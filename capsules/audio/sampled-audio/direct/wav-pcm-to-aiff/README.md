# wav-pcm-to-aiff

A standalone, zero-dependency Rust library that converts integer PCM carried
by RIFF/WAVE into classic AIFF. It owns its parser, writer, options, errors and
report, and has no dependency on EverythingX.

## Runnable default

```rust
use std::io::Cursor;

let mut input = Cursor::new(std::fs::read("input.wav")?);
let mut output = std::fs::File::create("output.aiff")?;
let report = wav_pcm_to_aiff::convert(
    &mut input,
    &mut output,
    &wav_pcm_to_aiff::Options::default(),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Defaults preserve the PCM sample sequence, map common RIFF INFO text fields,
strictly validate byte rate and block alignment, use a 64 KiB streaming buffer,
limit mapped metadata to 1 MiB, and limit channels to 256.

## Supported input

- RIFF/WAVE with chunks in arbitrary order;
- one or more frame-aligned `data` chunks;
- WAVE_FORMAT_PCM and WAVE_FORMAT_EXTENSIBLE with the PCM subtype;
- 8-, 16-, 24-, and 32-bit integer containers;
- extensible valid-bit widths from 1 through the container width;
- common INFO mappings: INAM→NAME, IART→AUTH, ICMT→ANNO, ICOP→`(c) `.

Eight-bit WAV samples are unsigned and become signed AIFF samples by toggling
the sign bit. Wider little-endian samples are emitted most-significant byte
first. Extensible samples with a narrower valid-bit width are left-aligned and
compacted to the AIFF sample width.

## Deliberate limits in 0.1

- floating-point WAV requires AIFC rather than this AIFF Capsule;
- RF64/BW64 cannot fit classic AIFF's 32-bit FORM and chunk sizes;
- RIFX is rejected in this version;
- cue points, broadcast extensions, channel masks and arbitrary metadata are
  reported but not migrated;
- the reader must seek because RIFF permits `fmt ` after `data`.

