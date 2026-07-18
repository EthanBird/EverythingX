# Catalog semantics

`source_records.ndjson` is one JSON object per upstream observation. It deliberately contains duplicate concepts across sources and conflicting extensions/media types. The indexes point from weak or external identifiers to all matching observations.

Never report `observation_count` as “the number of file formats in the world”. Canonical unique concepts live in `canonical/`, and each mapping requires evidence and review.

The catalog can be rebuilt deterministically from the pinned local snapshots with `tools/build_catalog.py`. Raw snapshots are not included in the distributable foundation artifact; their URLs and hashes are recorded in `sources/sources.json`.

