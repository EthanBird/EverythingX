# Required test classes

- Conformance fixtures against a public specification or trusted oracle.
- Property tests for declared invariants.
- Roundtrip or observational-equivalence tests appropriate to the contract.
- Regression fixtures for every accepted bug.
- Fuzzing for parsers and untrusted inputs.
- Benchmarks for throughput, latency, peak memory, output size, and quality metrics.

Every fixture must record origin, license/permission, checksum, and expected result.

