# Operator Universe

This directory is the versioned construction ledger between the open Format
Universe and executable Capsules.

- `operator-basis.json` defines the finite algebraic basis used to describe
  operations at every representation layer.
- `backlog.json` expands every Object IR × operator-kind position and every
  semantic-family × operator-family research cell.
- `audio/representations.json` is the reviewed audio representation snapshot.
- `audio/backlog.json` is generated from that snapshot and contains concrete,
  currently unimplemented candidate-edge sets.
- Implemented support is never recorded here; the authoritative implemented
  matrix is `registry/support-matrix.json`.

The four states must not be collapsed:

```text
observed format → candidate operator → computability reviewed → implemented capability
```

An absent implementation is `not-implemented`, not `impossible`. A generated
candidate is `unknown` until its invariants and target representability have
been reviewed.

The checked-in backlogs use set + expansion-rule + count + SHA-256 form rather
than repeating thousands of near-identical records. Fully materialized lists
remain deterministic:

```bash
python3 tools/build_operator_universe.py --materialize /tmp/operator-backlog.json
python3 tools/build_audio_backlog.py --materialize /tmp/audio-backlog.json
```
