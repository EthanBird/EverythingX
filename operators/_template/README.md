# Atomic operator template

Copy this directory only after the input/output semantic action can be named without the word “and”. `operator.json` is the graph contract; Rust code is one implementation of it.

Before changing code, define:

1. Which representation layer changes?
2. Which invariants are guaranteed?
3. Is a result extracted, decoded, rendered, or inferred?
4. What makes the partial function undefined?
5. What exact loss dimensions and bounds apply?
6. Which hostile-input and resource limits are enforced?
7. What evidence would falsify the lossless claim?

Do not put extension dispatch or multiple unrelated codecs in one operator. Backend alternatives may share the same semantic contract only if their guarantees and loss model are equal.

