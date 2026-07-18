# Project status — Architecture 2.0

Date: 2026-07-19  
Status: Architecture 2.0 implementation has started.

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

## First implementation slice

- `capsules/utf16-to-utf8`: zero-dependency, streaming standalone Capsule.
- Runnable defaults: BOM auto-detection, BOM-less little-endian, strict malformed-sequence rejection, no UTF-8 BOM, 64 KiB buffer.
- Strict and replace-invalid Adapter capabilities with explicit loss reporting.
- `kernel/ex-protocol`: handshake, capability, request/result, budget, loss and provenance types.
- `kernel/ex-kernel`: Adapter registration, default validation, direct discovery and direct invocation.
- End-to-end test: Kernel invokes the Capsule through Adapter using no caller-supplied options.

## Intentionally absent

- No claim that the first Capsule has completed fuzz and benchmark campaigns yet.
- No `ex-core` framework that converter libraries must implement.
- No mandatory shared IR.
- No CLI, desktop application or multi-step Planner.
- No claim that the template byte copy is a production conversion capability.

## Next milestone

Complete the remaining independent reference Capsules:

1. `bmp-to-png`
2. `wav-pcm-to-aiff`

In parallel, harden the current protocol and Kernel from real Adapter feedback without adding multi-step planning.
