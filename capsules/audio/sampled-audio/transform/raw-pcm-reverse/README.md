# raw-pcm-reverse

Frame-order reversal of parameterized raw interleaved PCM. This is an independent, zero-dependency Rust crate. Raw PCM
interpretation is explicit in `Options`; defaults are runnable and tests cover
alignment failures, parameter validation, default behavior and custom behavior.
