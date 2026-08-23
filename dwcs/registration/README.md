# DWCS registration artifact

This directory documents the registration boundary for DWCS, the Disagreement-Weighted Canonical Scoring module. DWCS is a deterministic Telegraph scoring module for `FRAUD_DETECTION` that evaluates a Miner answer against a ground-truth answer.

The source code is in [`../rust-module`](../rust-module). The compiled WASM binary is intentionally excluded from version control because registration depends on the exact reviewed build artifact.

## Runtime contract

The module exports `alloc`, `dealloc`, and `rank_answer`.

`rank_answer` receives three UTF-8 strings as pointer and length pairs:

1. `question`
2. `ground_truth`
3. `miner_answer`

It returns one `f32` score from `0` to `1`. The module has no network access, filesystem access, or persistent state.

## Scoring approach

DWCS combines four deterministic signals:

- normalized word overlap
- stopword-weighted overlap
- bigram Jaccard similarity
- longest-common-subsequence ratio

When these signals disagree sharply, DWCS dampens the final score. This reduces the value of answers that imitate correct vocabulary without preserving meaningful phrase structure.

## Verified build properties

The reviewed release build was produced for `wasm32-unknown-unknown` and validated with the following properties:

- nine Rust unit tests passed
- zero WASM imports
- required exports are present
- the official Telegraph `go-tester` loaded the binary successfully
- exact, wrong, empty, reworded, quality-ranked, Unicode, and long-input cases completed safely

The reviewed binary path was `dwcs/rust-module/target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm` with SHA-256 `B9FDA517680949B74A81E43839D42452F83E15A8B7150C6411C55AD4DC7F2A53`.

## Security boundary

No private keys, wallet files, payment material, or held-out canary data belong in this directory or in version control. The canary dataset remains local and gitignored so Miners cannot optimize against it.
