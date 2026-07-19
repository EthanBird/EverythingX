# raw-pcm-endian-signedness-normalize

Byte-order and integer signedness normalization of raw PCM samples. This is an independent, zero-dependency Rust crate. Raw PCM
interpretation is explicit in `Options`; defaults are runnable and tests cover
alignment failures, parameter validation, default behavior and custom behavior.
