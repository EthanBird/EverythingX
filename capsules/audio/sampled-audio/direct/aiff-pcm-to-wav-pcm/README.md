# aiff-pcm-to-wav-pcm

Standalone, zero-dependency conversion from classic AIFF integer PCM to
RIFF/WAVE integer PCM. It parses IFF chunk boundaries, COMM, SSND offsets and
exact integer 80-bit extended sample rates; converts signedness and byte order;
and emits WAVE_FORMAT_EXTENSIBLE when AIFF valid bits do not fill their byte
container.

Defaults strictly verify frame counts and FORM size, map NAME/AUTH/ANNO/(c)
text to LIST/INFO, use a 64 KiB stream buffer and reject unsupported AIFC,
fractional rates and classic RIFF size overflow.
