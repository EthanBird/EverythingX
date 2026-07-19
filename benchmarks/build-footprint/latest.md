# Rust build footprint

Generated: 2026-07-19T16:41:27.581395+00:00
Commit: `83d78a01233a74b21c0577294ff428a78fae91fe`
Runner: `Linux-6.17.0-1020-azure-x86_64-with-glibc2.39` / `x86_64` / 4 CPUs
Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`; `cargo 1.97.1 (c980f4866 2026-06-30)`

## Outcome

| Measurement | Value |
|---|---:|
| Unified all-capability release cold build | 61.315 s |
| Unified all-capability release no-op rebuild | 0.092 s |
| Kernel test compilation | 0.561 s |
| 104 independent Capsule release builds | 48.854 s |
| Unified release target tree | 79.80 MiB |
| Unified linked benchmark executable | 8.85 MiB |
| Stripped executable copy | 6.89 MiB |
| All isolated Capsule target trees | 44.88 MiB |
| All standalone Capsule `.rlib` artifacts | 25.65 MiB |
| Active Rust sysroot | 623.30 MiB |

The unified target size is developer build state, not a distributable package. It includes Cargo fingerprints, dependency metadata, libraries and the linked harness. The stripped executable is the closest number here to a monolithic release payload. A standalone Capsule is a library, so its `.rlib` is the relevant Rust artifact, not its entire isolated `target/` directory.

## Independent Capsule distribution

Per-Capsule cold release time: min 0.125 s, median 0.305 s, p95 0.951 s, max 0.975 s.

### Slowest builds

| Capsule | Seconds | `.rlib` | Target tree |
|---|---:|---:|---:|
| `image/raster/direct/png-to-bmp` | 0.975 | 383.15 KiB | 555.21 KiB |
| `image/raster/direct/png-to-pam` | 0.968 | 383.13 KiB | 555.13 KiB |
| `image/raster/direct/png-to-qoi` | 0.957 | 383.16 KiB | 555.22 KiB |
| `image/raster/direct/png-to-tga` | 0.955 | 383.16 KiB | 555.22 KiB |
| `image/raster/direct/tga-to-png` | 0.953 | 383.16 KiB | 555.22 KiB |
| `image/raster/direct/ppm-to-png` | 0.951 | 383.13 KiB | 555.20 KiB |
| `image/raster/direct/pam-to-png` | 0.943 | 383.15 KiB | 555.14 KiB |
| `image/raster/direct/png-to-ppm` | 0.942 | 383.15 KiB | 555.21 KiB |
| `image/raster/direct/qoi-to-png` | 0.941 | 383.15 KiB | 555.21 KiB |
| `image/raster/transform/png-flip-vertical` | 0.743 | 227.12 KiB | 376.22 KiB |

### Largest standalone libraries

| Capsule | `.rlib` | Source without Adapter | Seconds |
|---|---:|---:|---:|
| `image/raster/direct/bmp-to-png` | 390.08 KiB | 69.32 KiB | 0.285 |
| `image/raster/direct/png-to-tga` | 383.16 KiB | 79.37 KiB | 0.955 |
| `image/raster/direct/png-to-qoi` | 383.16 KiB | 79.33 KiB | 0.957 |
| `image/raster/direct/tga-to-png` | 383.16 KiB | 79.37 KiB | 0.953 |
| `image/raster/direct/pam-to-png` | 383.15 KiB | 79.30 KiB | 0.943 |
| `image/raster/direct/png-to-bmp` | 383.15 KiB | 79.36 KiB | 0.975 |
| `image/raster/direct/png-to-ppm` | 383.15 KiB | 79.31 KiB | 0.942 |
| `image/raster/direct/qoi-to-png` | 383.15 KiB | 79.32 KiB | 0.941 |
| `image/raster/direct/png-to-pam` | 383.13 KiB | 79.30 KiB | 0.968 |
| `image/raster/direct/ppm-to-png` | 383.13 KiB | 79.31 KiB | 0.951 |

## Unified target breakdown

| Class | Files | Logical size |
|---|---:|---:|
| cargo-metadata | 918 | 380.24 KiB |
| dependency-info | 212 | 217.12 KiB |
| executable | 1 | 8.85 MiB |
| other | 4 | 177.00 B |
| rlib | 210 | 50.36 MiB |
| rmeta | 210 | 20.01 MiB |

## Method

- Stable Rust with the minimal rustup profile on a fresh GitHub-hosted Ubuntu runner.
- Every cold measurement uses a new `CARGO_TARGET_DIR` under the ephemeral runner temp directory.
- The no-op rebuild repeats the identical release command against the populated unified target.
- Each production Capsule is then built independently with its own isolated release target and locked manifest.
- Logical and allocated totals deduplicate Cargo hardlinks by filesystem inode; `path_sum_bytes` in JSON retains the non-deduplicated audit value.
- Network dependency download time is absent because the current production Capsules are dependency-free.
