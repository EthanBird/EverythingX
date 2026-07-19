# Capsule taxonomy

Every production Capsule lives at:

```text
capsules/<domain>/<primary-object-ir>/<operator-role>/<capsule-name>/
```

For example, `audio/sampled-audio/direct/wav-pcm-to-aiff` says that the
Capsule belongs to the audio domain, computes primarily over the
`ir:sampled-audio` Object IR, and performs a direct representation-to-
representation operation.

The directory is an index, not a dependency boundary. Each leaf remains a
standalone Rust crate with its own manifest, API, defaults, tests, validation
record, licence and optional `everythingx/` Adapter. Copying the leaf out of
this repository and deleting its Adapter must leave a working crate.

## Levels

- **domain** groups human and engineering ownership: audio, image, text,
  document, data, archive, geometry, scientific, executable, and future
  domains.
- **primary Object IR** is the semantic state the algorithm preserves or
  transforms. Secondary IRs are recorded in `capsule.json`; they do not create
  additional physical copies of a Capsule.
- **operator role** describes graph position: `direct`, `decode`, `encode`,
  `transform`, `split`, `aggregate`, `analyze`, `bridge`, or `structural`.
- **capsule name** is the independently runnable algorithm crate.

`capsule.json.taxonomy` is authoritative and CI verifies that its domain,
primary IR and role agree with the first three directory levels. Tooling must
discover manifests recursively; adding a new Capsule must not require editing
a hard-coded CI path list.

`_template` is intentionally outside the production taxonomy and is excluded
from the implemented support matrix.
