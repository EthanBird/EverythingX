# Rust build footprint

Generated: 2026-07-19T16:37:28.384513+00:00
Commit: `2993a3fdb58d0eac4ec42449b62bfd84f82d42bc`
Runner: `Linux-6.17.0-1020-azure-x86_64-with-glibc2.39` / `x86_64` / 4 CPUs
Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`; `cargo 1.97.1 (c980f4866 2026-06-30)`

## Outcome

| Measurement | Value |
|---|---:|
| Unified all-capability release cold build | 62.327 s |
| Unified all-capability release no-op rebuild | 0.087 s |
| Kernel test compilation | 0.540 s |
| 104 independent Capsule release builds | 46.617 s |
| Unified release target tree | 88.66 MiB |
| Unified linked benchmark executable | 8.85 MiB |
| Stripped executable copy | 6.89 MiB |
| All isolated Capsule target trees | 70.53 MiB |
| All standalone Capsule `.rlib` artifacts | 25.65 MiB |
| Active Rust sysroot | 623.30 MiB |

The unified target size is developer build state, not a distributable package. It includes Cargo fingerprints, dependency metadata, libraries and the linked harness. The stripped executable is the closest number here to a monolithic release payload. A standalone Capsule is a library, so its `.rlib` is the relevant Rust artifact, not its entire isolated `target/` directory.

## Independent Capsule distribution

Per-Capsule cold release time: min 0.117 s, median 0.291 s, p95 0.923 s, max 0.930 s.

### Slowest builds

| Capsule | Seconds | `.rlib` | Target tree |
|---|---:|---:|---:|
| `image/raster/direct/png-to-qoi` | 0.930 | 383.16 KiB | 938.38 KiB |
| `image/raster/direct/png-to-pam` | 0.929 | 383.13 KiB | 938.26 KiB |
| `image/raster/direct/png-to-bmp` | 0.926 | 383.15 KiB | 938.36 KiB |
| `image/raster/direct/ppm-to-png` | 0.925 | 383.13 KiB | 938.33 KiB |
| `image/raster/direct/qoi-to-png` | 0.924 | 383.15 KiB | 938.36 KiB |
| `image/raster/direct/pam-to-png` | 0.923 | 383.15 KiB | 938.29 KiB |
| `image/raster/direct/png-to-ppm` | 0.922 | 383.15 KiB | 938.36 KiB |
| `image/raster/direct/png-to-tga` | 0.919 | 383.16 KiB | 938.38 KiB |
| `image/raster/direct/tga-to-png` | 0.909 | 383.16 KiB | 938.38 KiB |
| `image/raster/transform/png-flip-vertical` | 0.707 | 227.12 KiB | 603.34 KiB |

### Largest standalone libraries

| Capsule | `.rlib` | Source without Adapter | Seconds |
|---|---:|---:|---:|
| `image/raster/direct/bmp-to-png` | 390.08 KiB | 69.32 KiB | 0.269 |
| `image/raster/direct/png-to-tga` | 383.16 KiB | 79.37 KiB | 0.919 |
| `image/raster/direct/png-to-qoi` | 383.16 KiB | 79.33 KiB | 0.930 |
| `image/raster/direct/tga-to-png` | 383.16 KiB | 79.37 KiB | 0.909 |
| `image/raster/direct/pam-to-png` | 383.15 KiB | 79.30 KiB | 0.923 |
| `image/raster/direct/png-to-bmp` | 383.15 KiB | 79.36 KiB | 0.926 |
| `image/raster/direct/png-to-ppm` | 383.15 KiB | 79.31 KiB | 0.922 |
| `image/raster/direct/qoi-to-png` | 383.15 KiB | 79.32 KiB | 0.924 |
| `image/raster/direct/png-to-pam` | 383.13 KiB | 79.30 KiB | 0.929 |
| `image/raster/direct/ppm-to-png` | 383.13 KiB | 79.31 KiB | 0.925 |

## Unified target breakdown

| Class | Files | Logical size |
|---|---:|---:|
| cargo-metadata | 918 | 380.24 KiB |
| dependency-info | 212 | 217.12 KiB |
| executable | 2 | 17.71 MiB |
| other | 4 | 177.00 B |
| rlib | 210 | 50.36 MiB |
| rmeta | 210 | 20.01 MiB |

## Method

- Stable Rust with the minimal rustup profile on a fresh GitHub-hosted Ubuntu runner.
- Every cold measurement uses a new `CARGO_TARGET_DIR` under the ephemeral runner temp directory.
- The no-op rebuild repeats the identical release command against the populated unified target.
- Each production Capsule is then built independently with its own isolated release target and locked manifest.
- Logical byte counts sum file lengths; allocated byte counts use filesystem block accounting.
- Network dependency download time is absent because the current production Capsules are dependency-free.
