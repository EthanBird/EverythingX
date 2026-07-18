# Standalone Conversion Capsule template

This directory is intentionally buildable without EverythingX. Copy it outside the repository and run:

```bash
cargo test
```

The core library owns its `Options`, `Error`, and `Report`. It must not import EverythingX types. The optional `everythingx/` directory is the only integration surface and may be deleted without changing core behavior.

Before creating a production Capsule:

1. Replace the example byte copy with one named conversion.
2. Replace the template license with a complete standalone license.
3. Define exact source/target scope and unsupported profiles.
4. Add specifications, corpus provenance, differential oracles and fuzzing.
5. Add reproducible quality/performance benchmarks.
6. Report actual strategy, backend, losses and warnings.
7. Verify no path dependency escapes the Capsule root.

