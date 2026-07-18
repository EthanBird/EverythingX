# Project status — Architecture 2.0

Date: 2026-07-19  
Status: design reset complete; implementation has not started.

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

## Intentionally absent

- No production Capsule yet.
- No `ex-core` framework that converter libraries must implement.
- No mandatory shared IR.
- No CLI, desktop application or multi-step Planner.
- No claim that the template byte copy is a production conversion capability.

## Next milestone

Build three independent reference Capsules:

1. `utf16-to-utf8`
2. `bmp-to-png`
3. `wav-pcm-to-aiff`

Only after all three pass copy-out build/test/bench/fuzz should the minimal Adapter Protocol and Kernel runtime be implemented.

