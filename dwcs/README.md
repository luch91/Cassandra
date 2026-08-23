# DWCS: Disagreement-Weighted Canonical Scoring

Script Author track entry. Full design rationale in `../docs/PROJECT_SPEC.md` Section 4. Architecture confirmed and corrected Aug 22, see decision D9, this is the real, buildable version.

## The real interface (confirmed, not assumed)

Source: `https://docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module`

- The scoring module is a sandboxed WASM binary. **No network access, no filesystem access, no shared state across calls.**
- It receives exactly three plain-text strings: `question`, `ground_truth`, `miner_answer`, as `(pointer, length)` pairs.
- It exports `alloc`, `dealloc`, `rank_answer`. `rank_answer` returns a single `f32` between 0 and 1. No struct, no confidence field, nothing else.
- Target `wasm32-unknown-unknown`, 32MB size cap.
- Promotion is two-stage: Stage 1 structural checks, then Stage 2, beat the current champion module on a fixed benchmark (margin, win-count, self-match strength).

## Directory layout

- **`rust-module/`**, the real, deployable implementation. This is what actually gets compiled to `.wasm` and registered on Telegraph. See its own README for build/test/register steps.
- **`src/prototype.ts`**, a TypeScript mirror of the same scoring logic, for fast iteration on metric weights and thresholds before porting a change into the Rust source. **Not the deployed module.** If you change something here, port the equivalent change to `rust-module/src/lib.rs`, don't let the two drift apart.
- **`canaries/`**, held-out adversarial `(question, ground_truth, miner_answer)` triples used to demonstrate gaming-resistance. Never expose to Miners or the public network.
- **`tests/`**, Jest tests for `src/prototype.ts`. The Rust module has its own unit tests inline in `rust-module/src/lib.rs`, runnable with plain `cargo test` on the host, no WASM tooling needed for that.

## The mechanism, in short

Instead of one naive similarity check (Telegraph's own reference example is a plain word-overlap scorer, explicitly framed as "a legitimate starting point," meaning most entrants will ship exactly that), DWCS computes four structurally different, purely algorithmic similarity signals between `ground_truth` and `miner_answer`:

1. Normalized word overlap (the naive baseline, included as one input, not the whole story)
2. Stopword-down-weighted overlap (can't inflate score by padding with filler words)
3. Bigram Jaccard similarity (catches phrase-level structure, resists keyword-scrambling)
4. Longest common subsequence ratio over words (catches valid paraphrases, the exact case Telegraph's own docs name as something "a good scorer should still recognize... as correct")

Low variance across the four means trust the combined score. High variance means an answer likely games one metric while failing the others, so the final score gets damped toward a conservative blend rather than rewarded. That damped value **is** the single `f32` returned, there's nowhere else to put a separate confidence signal.

## Build order

1. `rust-module/src/lib.rs`, buildable and testable right now with `cargo test`, no Telegraph-specific unknowns remain.
2. `canaries/dataset.jsonl`, populate with real, reviewed adversarial examples (see `canaries/README.md`), not the placeholder content in `dataset.example.jsonl`.
3. Local benchmark replication, build a small internal benchmark set (good/bad answer pairs per question) and check self-match ≥ 0.75 and real score variance, matching Telegraph's own Stage 2 criteria, before ever registering on-chain.
4. Register via `integrate.telegraphprotocol.com` (hosts and hashes the file for you) or by calling `registerWasm(wasmHash, wasmUrl, intent)` directly on the Diamond contract.

## Running tests

```
npm run test:dwcs      # TS prototype tests
cd rust-module && cargo test    # real module's own unit tests, run on host, no WASM tooling needed
```
