# raw-pcm-to-wav-pcm

A standalone, zero-dependency Rust library that gives finite interleaved raw
integer PCM a canonical RIFF/WAVE PCM container. It can be copied out of
EverythingX and built independently.

```rust
let report = raw_pcm_to_wav_pcm::convert(
    &mut input,
    &mut output,
    &raw_pcm_to_wav_pcm::Options::default(),
)?;
```

Defaults interpret input as mono 44.1 kHz signed 16-bit little-endian PCM.
Options explicitly own the information a headerless stream cannot carry:
channels, sample rate, width, byte order and signedness. Accepted widths are
8, 16, 24 and 32 bits. Output is classic little-endian WAVE PCM; signedness is
rebased when raw and WAVE conventions differ.

Strict defaults reject incomplete frames. A relaxed option can discard a
trailing partial frame and reports that loss. Classic RIFF's 32-bit data-size
limit is enforced before output begins.
