# Image operator program

Date: 2026-07-19  
Status: open-world representation ledger established; Raster Wave A and PNG Wave B implemented.

## 1. Boundary of the image family

“Image” is not one file class. It is a family of materializations whose common
purpose is visual representation, but whose computable objects may be very
different:

| Representation family | Primary Object IR | What must remain meaningful |
|---|---|---|
| Pixel raster | `ir:raster` | grid, coordinates, samples, channels, alpha, color interpretation |
| Camera raw | `ir:raster` plus calibration facts | sensor samples, mosaic, black/white levels, lens and color calibration |
| Scientific/medical volume | `ir:tensor` | dimensions, axes, units, sample type, spatial calibration and domain metadata |
| Layered authoring project | `ir:object-graph` | layers, masks, effects, blend graph, editability and embedded resources |
| Vector/page graphics | `ir:vector` | geometry, paints, text, clipping, coordinate systems and executable drawing semantics |
| Animation or pyramid | `ir:timed-media` or collection | frame timing, disposal, ordering, levels, tiles and spatial addressing |
| Texture/GPU resource | `ir:raster` plus hardware profile | block layout, mip levels, faces, arrays, transfer semantics and GPU format |

Therefore a file-name edge such as `PSD → PNG` is not an ordinary container
rewrite. It is a render/flatten operation from an object graph to one raster,
while `PNG → QOI` can be a conditional exact conversion of decoded pixel code
values. These operations must not share one unqualified `convert` promise.

## 2. Open-world representation ledger

`operators/image/representations.json` currently reviews 298 representations in
nine domains. It deliberately distinguishes:

- codec bitstream from container profile;
- still image from animation or collection;
- raw sensor state from rendered pixels;
- a layered project from its flattened preview;
- pixel values from their color interpretation;
- a whole file from a member, frame, tile, mip level or glyph strike.

The snapshot is not advertised as the final count of world image formats. New
camera models, GPU formats, private microscopy containers and operational
profiles keep the universe open. The list is a versioned research boundary that
can grow without changing the Capsule architecture.

The 68 operator templates include identification, validation, repair, pairwise
conversion, decoding/encoding, member and frame algebra, spatial transforms,
color and alpha operations, raw development, compositing, rasterization,
vectorization, restoration, analysis and OCR. The generated backlog contains
11,234 ordered pair positions. Every position begins with unknown computability;
being present in the backlog is never a support claim.

## 3. Raster computability predicate

For a direct raster carrier conversion `A → B`, exactness requires more than
matching width and height. The initial predicate is:

```text
compatible(A, B, options) :=
    source parser recognizes a bounded single image
  ∧ target can represent its dimensions
  ∧ target can represent every accepted channel and sample value
  ∧ alpha handling is explicit
  ∧ coordinate order can be normalized without resampling
  ∧ intended color semantics are compatible or an explicit transform is chosen
```

The invariant vector recorded by a Capsule is:

```text
(width, height, pixel coordinates, channel code values,
 alpha association, sample depth, color interpretation, metadata subset)
```

Loss is evaluated independently on pixels, coordinates, alpha, color semantics,
structure and metadata. A converter may be exact on pixel code values while
remaining conditional on color interpretation and unbounded on metadata.

## 4. Raster Wave A

Wave A closes the complete directed graph among five structurally tractable
single-image carriers:

```text
BMP ↔ TGA ↔ QOI ↔ PPM ↔ PAM
 \________________________/
     all 20 directed edges
```

Every edge is a standalone, zero-dependency Rust crate under
`capsules/image/raster/direct/`. Copying one leaf elsewhere and deleting its
optional `everythingx/` directory leaves a complete library with its own API,
defaults, errors, report, tests, lockfile, license and conformance fixture.

The materialized implementation owns native parsers and encoders for:

- QOI 1.0 index, diff, luma, run, RGB and RGBA chunks;
- TGA raw/RLE truecolor and grayscale with all four origin directions;
- BMP 24-bit BI_RGB and 32-bit BI_RGB or explicit BGRA8 V4 masks;
- PPM P3/P6 and PAM visual tuple parsing, with MAXVAL 255 under strict defaults;
- alpha-preserving QOI/TGA/BMP/PAM output and an explicit PPM alpha policy.

The internal RGBA8 proof model is copied into each crate. It is not a shared
runtime dependency, a Kernel type or a mandatory universal IR. Runnable defaults
reject non-opaque pixels when the target is PPM; callers must explicitly choose
discard or black compositing to permit that controlled loss.

## 5. Evidence and current optimization state

Wave A adds 80 standalone core tests and 20 Kernel/Adapter default-invocation
tests. CI also copies all production Capsules out of the repository, deletes the
Adapter, and runs their locked Cargo tests independently.

The controlled `exbench:ci-default-v1` run invokes every capability through the
Kernel on deterministic small and large valid inputs. The 20 new edges measure
129.723–290.495 MiB/s on the current GitHub runner. Their reported peak-memory
models are 3.75–4.67 input bytes per input byte because version 0.1 validates a
complete input, decoded pixel buffer and encoded output before committing the
target.

This is a correctness-first baseline, not a claim of global optimality. The next
optimization gate is profile-specialized row streaming for compatible source
and target order, retaining full buffering only for operations that need random
access, orientation reversal or atomic output validation. Performance evidence
will decide graph cost and optimization priority; it never overrides semantic
preconditions or loss.

## 6. PNG Wave B

Audio Wave B remains paused. PNG Wave B delivered:

1. nine missing PNG directions against BMP/TGA/QOI/PPM/PAM
   (the existing specialized BMP→PNG edge remains);
2. independent strict PNG validation and deterministic structural normalization;
3. crop, pad, horizontal/vertical flip, 90/180/270-degree rotation,
   alpha premultiply and alpha unpremultiply Capsules;
4. a native PNG decoder for legal color/depth combinations, PLTE/tRNS,
   all Deflate block types, five filters and Adam7;
5. a canonical RGB/RGBA 8/16-bit encoder with adaptive filters, stored Deflate,
   CRC-32, Adler-32 and deterministic 64 KiB IDAT chunking.

The 20-Capsule wave closes a declared format subgraph and a coherent transform
basis instead of scattering isolated edges. Direct fixed/dynamic Huffman,
Adam7, sub-byte indexed transparency and 16-bit round-trip tests are copied into
every leaf; the generator is development tooling, not a runtime dependency.

## 7. Next common-format waves

PNG Wave B does not justify claiming that common image formats are complete.
The next ordered program is:

1. GIF87a/GIF89a and ICO/CUR, including frame/member algebra and explicit
   animation/selection semantics;
2. baseline/progressive JPEG and TIFF profile families, with color and metadata
   contracts separated from pixel-code-value claims;
3. WebP lossy/lossless/animation and AVIF/HEIF/HEIC item/sequence graphs;
4. SVG and PDF rasterization bridges as object-graph render operations, never
   mislabeled as carrier-only conversions;
5. camera raw development only after calibration, demosaic and color-transform
   preconditions are represented in capability records.

Each codec receives a native-versus-dependency decision record. “Zero dependency”
does not permit a partial decoder to masquerade as common-format support; where
the codec state space is too large for the current native proof, a dependency
backend remains preferable to a false support claim.

HEIF is now explicitly expanded into HEVC, HEIC, AVC, VVC, EVC, JPEG,
uncompressed and AVIF-related representations rather than one ambiguous file
extension. The machine-readable H0/H1/H2 construction program specifies 58
independent Capsules: 20 native container-graph operators, 20 HEIC still-image
pixel edges and 18 additional codec-profile/sequence operators. This is a plan,
not an implemented-support claim; details and runnable defaults are in
`operators/image/heif-heic-program.json` and `docs/16-heif-heic-program.md`.
