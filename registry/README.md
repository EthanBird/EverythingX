# Registry boundary

This directory will hold CapsuleRelease, AdapterRelease, CapabilityBinding, benchmark and trust assertions. Format-universe facts remain in `catalog/`, `canonical/` and `ontology/`.

Registry snapshots are inputs to the Kernel, never compile-time dependencies of standalone Capsules.

`support-matrix.json` is the checked-in, machine-readable answer to “what can
EverythingX actually convert today?”. It is derived only from non-template
Capsule and Adapter manifests:

```bash
python3 tools/build_support_matrix.py
python3 tools/build_support_matrix.py --check
```

Every implementation update must regenerate it. Planned and merely researched
operators belong under `operators/`; they must never be presented as supported.
