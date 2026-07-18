# Kernel boundary

The first thin Kernel implementation now exists as two zero-dependency crates:

- `ex-protocol`: handshake, capability descriptors, runnable defaults, invocation request/result, loss and provenance types, plus the static Adapter boundary.
- `ex-kernel`: Adapter registration, protocol/default validation, direct capability discovery and direct invocation.

It intentionally has no multi-step planner, format parser, mandatory IR or Capsule-facing trait. Capsule code remains unaware of both crates.

See `ARCHITECTURE.md` and `docs/08-kernel-and-adapters.md`.
