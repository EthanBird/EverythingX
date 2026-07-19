# Image operator program

Date: 2026-07-19  
Status: open-world representation ledger established; Raster Wave A implemented.

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

`operators/image/representations.json` currently reviews 295 representations in
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
10,838 ordered pair positions. Every position begins with unknown computability;
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

## 6. Next image wave

Audio Wave B is paused. The next image batch is the PNG-centered compressed
raster wave:

1. implement the nine missing PNG directions against BMP/TGA/QOI/PPM/PAM
   (the existing specialized BMP→PNG edge remains);
2. add independent PNG validation and deterministic structural normalization;
3. add crop, pad, horizontal/vertical flip, 90/180/270-degree rotation,
   alpha premultiply and alpha unpremultiply Capsules;
4. differential-test PNG decoding/encoding against official conformance data
   before moving to JPEG, WebP, GIF, AVIF/HEIF and camera raw.

This produces a 20-plus Capsule batch while closing a declared format subgraph
and a coherent transform basis instead of scattering isolated edges.
