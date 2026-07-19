# aiff-pcm-to-bwf-pcm

Independent, zero-dependency Rust conversion from classic AIFF PCM to
Broadcast WAVE PCM. The crate parses the source container, validates integer
PCM structure, emits the target container natively and streams sample frames
through a bounded buffer. Copy this directory elsewhere and it remains a
complete library with runnable defaults and unit tests.

Version 0.1 supports interleaved integer PCM at 8/16/24/32 bits. It preserves
sample rate, channel count, frame order and sample levels; unsupported metadata
is reported as a declared boundary rather than silently advertised as retained.
