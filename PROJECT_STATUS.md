# Project status — Architecture 2.0

Date: 2026-07-19  
Status: Reference architecture validated; operator-universe construction and audio-family program active.

## Accepted architecture

- Conversion Capsule is the first-class implementation and release unit.
- Every Capsule must build, test and run independently of EverythingX.
- EverythingX integration lives only in a removable Adapter directory.
- Kernel is a control plane for registry, policy, invocation, proof, provenance and planning.
- Graph edges are AdapterCapability records, not implementation traits.
- Family IR is optional and independently versioned, never a mandatory Kernel type.
- Direct specialized converters and generic composed paths coexist.
- Format Universe remains a versioned open world with domain and private namespaces.

## Current data foundation

| Metric | Count |
|---|---:|
| Source observations | 9,020 |
| IANA observations | 2,319 |
| PRONOM/DROID observations | 2,557 |
| Library of Congress FDD observations | 596 |
| Apache Tika observations | 1,695 |
| freedesktop observations | 1,038 |
| GitHub Linguist observations | 815 |
| Distinct media-type labels | 3,972 |
| Distinct extension strings | 4,036 |
| Distinct external identifiers | 12,175 |

These are source observations, not a count of unique canonical formats.

## Operator and support ledgers

| Metric | Count |
|---|---:|
| Actually implemented standalone Capsules | 42 |
| Actually implemented Adapter capabilities | 43 |
| Actually implemented logical format pairs | 39 |
| Object IR × operator-kind research positions | 4,743 |
| Semantic-family × operator-family research cells | 310 |
| Reviewed audio representations | 172 |
| Audio operator templates | 42 |
| Generated ordered audio pair candidates | 8,672 |
| Actually implemented distinct-format audio pairs | 36 |
| Capsules covered by performance harness | 42 |
| Capability edges covered by performance harness | 43 |

`registry/support-matrix.json` is generated from real manifests and answers what works now. `operators/audio/backlog.json` is a research and implementation queue; its candidate count is not a feature count.

## Delivered in Architecture 2.0

- `ARCHITECTURE.md`
- Conversion Capsule specification
- Kernel and Adapter boundary specification
- Universe-scale registry outlook
- HEIC→JPEG gold Capsule reference design
- Capsule and Adapter JSON schemas
- Standalone Rust Capsule template
- Removable static Adapter template
- Capsule independence checker
- CI proof that the copied Capsule builds after deleting EverythingX integration
- Revised governance, ADR and development roadmap
- Finite cross-family operator basis and 31 Object IR positions
- Generated 4,743 Object IR/operator research positions and 310 family research cells
- Generated implemented-support matrix with CI freshness enforcement
- Audio representation universe and complete ordered-pair research backlog
- Recursive Capsule taxonomy by domain, primary Object IR and operator role
- Automatic recursive discovery for manifests, copy-out tests and Adapter tests

## Implemented Capsules

- `capsules/text/byte-coding/direct/utf16-to-utf8`: zero-dependency, streaming standalone Capsule.
- Runnable defaults: BOM auto-detection, BOM-less little-endian, strict malformed-sequence rejection, no UTF-8 BOM, 64 KiB buffer.
- Strict and replace-invalid Adapter capabilities with explicit loss reporting.
- `kernel/ex-protocol`: handshake, capability, request/result, budget, loss and provenance types.
- `kernel/ex-kernel`: Adapter registration, default validation, direct discovery and direct invocation.
- End-to-end test: Kernel invokes the Capsule through Adapter using no caller-supplied options.

### `bmp-to-png` 0.1 implementation

- Zero-dependency Windows BMP parser and PNG encoder.
- 1/4/8-bit palettes, 16/32-bit bitfields, 24/32-bit pixels, top-down and bottom-up rows.
- BI_RGB, BI_BITFIELDS, BI_ALPHABITFIELDS, BI_RLE4 and BI_RLE8 input paths.
- Native PNG filters, chunked IDAT, CRC-32, Adler-32, stored Deflate and fixed-Huffman/RLE Deflate.
- Runnable defaults: adaptive filter, fixed-RLE compression, undeclared alpha normalized to opaque, 64 KiB IDAT chunks and 100-million-pixel limit.
- Test-side PNG inflater and unfilter implementation verifies emitted pixels without a third-party codec.
- Static Adapter buffers the input to bridge the Kernel's forward-only reader to the Capsule's seekable API.
- GitHub CI passes 12 standalone behavior tests, copy-out build after Adapter deletion, and Kernel default invocation.

### `wav-pcm-to-aiff` 0.1 implementation

- Zero-dependency RIFF/WAVE scanner and classic AIFF writer with streaming PCM emission.
- Integer PCM 8/16/24/32-bit containers, unsigned 8-bit conversion, multi-byte endianness conversion and frame/channel-order preservation.
- WAVE_FORMAT_EXTENSIBLE integer PCM with narrower valid-bit compaction.
- Arbitrary chunk order, multiple frame-aligned `data` chunks, even-byte padding and strict/relaxed header consistency.
- Exact AIFF 80-bit extended encoding for integer sample rates.
- Common LIST/INFO metadata mapping to NAME, AUTH, ANNO and `(c) ` with an explicit discard policy and byte limit.
- Explicit classic-container boundaries for RF64/BW64, RIFX, compressed/float WAV, 32-bit frame counts and AIFF FORM/SSND sizes.
- Static Adapter accounts for its seekable input buffer together with Capsule working memory and enforces the output budget.
- Standalone behavior tests cover PCM widths, metadata, chunk graph, malformed headers, fragmented reads and Kernel default invocation.
- GitHub CI passes 12 standalone behavior tests, copy-out build after Adapter deletion, and Kernel default invocation.

### PCM interchange batch 0.1

- `aiff-pcm-to-wav-pcm`: classic AIFF parser, exact integer 80-bit sample rates, SSND offsets, 1–32 valid bits, WAVE_FORMAT_EXTENSIBLE output and common text metadata mapping.
- `raw-pcm-to-wav-pcm`: explicit headerless-PCM contract with runnable mono/44.1 kHz/signed-16-LE defaults; 8/16/24/32-bit byte order and signedness normalization.
- `wav-pcm-to-raw-pcm`: PCM/extensible RIFF scanner, multiple data-chunk concatenation and signed/unsigned little/big-endian raw output with interpretation facts in the report.
- All three are zero-dependency standalone Rust crates with their own Options, Error, Report, tests, manifests, defaults and removable static Adapters.
- Together with `wav-pcm-to-aiff`, the implemented audio ledger now contains four real pair edges: AIFF↔WAV and raw PCM↔WAV.

### PCM Wave A batch 0.2 — 16 Capsules

- Twelve bidirectional container Capsules complete WAV↔CAF, WAV↔AU, WAV↔RF64, WAV↔BW64, WAV↔Wave64 and WAV↔BWF.
- Four parameter-owned raw PCM Capsules implement trim, frame reverse, channel projection/reordering and endian/signedness normalization.
- Every new Capsule contains four core unit tests: default behavior, malformed or partial input, option/resource validation and custom semantics.
- Every new static Adapter contains a Kernel default-invocation test, for 80 newly introduced core and integration tests across the batch.
- The development-only generator is freshness-checked, but each generated leaf contains complete source and has no runtime or path dependency outside itself after deleting `everythingx/`.

### PCM direct mesh batch 0.3 — 20 Capsules

- Ten directed Capsules complete CAF↔AU, CAF↔RF64, CAF↔BW64, CAF↔Wave64 and CAF↔BWF.
- Eight directed Capsules complete AU↔RF64, AU↔BW64, AU↔Wave64 and AU↔BWF.
- Two directed Capsules complete RF64↔BW64.
- These are direct parsers and emitters for their declared endpoints; they do not materialize a WAV intermediary.
- Every Capsule retains its own complete source, manifest, defaults, lockfile, license, tests, fuzz target and benchmark target, with no external runtime or path dependency.
- The batch adds 80 core unit tests and 20 Kernel/Adapter default-invocation tests.

### Performance evidence 0.1

- One generated release-mode harness recursively covers all 42 production Capsules and all 43 Adapter capabilities.
- Every capability is invoked through the Kernel with runnable defaults on deterministic small and large valid inputs.
- Evidence records p50/p95 latency, throughput, output ratio and Capsule-reported working memory together with compiler, environment, commit and harness identity.
- Planner-facing records expose a size-sensitive raw cost model; a calibrated 0–100 geometric score is retained only for same-profile ranking and UI summaries.
- CI first runs functional, copy-out and Adapter tests, then rejects missing benchmark coverage or failed benchmark invocations.
- The first controlled `ubuntu-24.04`/x86-64 baseline covers 42 Capsules and 43 capabilities: median large-workload throughput is 2,750.649 MiB/s; observed range is 27.912–3,702.017 MiB/s and calibrated score range is 14.190–58.040.
- The measured low-throughput priorities are the current native BMP→PNG implementation (27.912 MiB/s), UTF-16→UTF-8 strategies (about 159 MiB/s) and raw PCM reverse (217.572 MiB/s); these values are evidence for optimization work, not correctness or quality rankings.

## Intentionally absent

- No claim that the first Capsule has completed fuzz and benchmark campaigns yet.
- No `ex-core` framework that converter libraries must implement.
- No mandatory shared IR.
- No CLI, desktop application or multi-step Planner.
- No claim that the template byte copy is a production conversion capability.

## Next milestone

The three-Capsule reference gate is complete. Audio is the active implementation family:

1. Harden all 42 production Capsules with corpus manifests, fuzz targets and reproducible benchmark reports.
2. Implement the next 22-Capsule batch: the remaining ten non-WAV PCM mesh directions plus twelve AIFF↔CAF/AU/RF64/BW64/Wave64/BWF directions, completing all 56 directed edges among eight PCM containers.
3. Close the remaining PCM Wave A multi-input/output work: channel split/aggregate and concatenate, which first requires an n:m Adapter transport contract.
4. Continue through lossless codecs, container/essence operations, lossy codecs, signal transforms, musical events, sessions, banks and spatial audio according to `docs/12-audio-operator-program.md`.
5. Do not switch to isolated Text/Table operators until the active audio wave has reached its declared closure gate.
