# wav-pcm-to-raw-pcm

Standalone, zero-dependency extraction of interleaved integer PCM from
RIFF/WAVE. Default output is signed little-endian raw PCM; the `Report` carries
channels, rate, container bits, valid bits and frame count so the headerless
result remains interpretable.

The parser accepts WAVE_FORMAT_PCM and integer WAVE_FORMAT_EXTENSIBLE,
arbitrary chunk order and multiple aligned `data` chunks. It rejects compressed
and floating-point WAVE, RIFX and inconsistent strict headers. Output byte order
and signedness are configurable without changing sample values.
