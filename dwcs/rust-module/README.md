# dwcs-scoring-module

The real, deployable DWCS implementation. The production entry point follows Telegraph's published baseline: MiniLM-L6-v2 semantic similarity, BM25 lexical overlap, and length quality combined into one score. The module also exposes the cached and diagnostic functions used by the validator integration.

## Test on the host (no WASM tooling needed)

```
cargo test
```

The crate is `no_std` and the panic handler is disabled only for host tests. This runs the tokenizer, embedding, BM25, and contradiction tests without requiring a WASM runtime.

## Build the real WASM binary

```
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Output: `target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm`

**Verify it has zero imports before registering, this is non-negotiable:**

```
wasm-tools print target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm | grep -c '(import'
```

Must print `0`. If it doesn't, you likely built or copied the wrong target (check you're not accidentally using a `wasm32-wasip1` build, that target has OS-function imports and will fail to instantiate on Telegraph's node).

## Test against Telegraph's own test harness before registering

Clone `telegraph-examples/wasm-scoring-module/go-tester` and run:

```
cd go-tester
go run . dwcs_scoring_module.wasm \
  "What is the capital of France?" \
  "Paris is the capital of France." \
  "The capital of France is Paris."
```

Run this with at least: the exact correct answer, a clearly wrong/unrelated answer, an empty string, a reworded correct answer, and a couple of answers of different quality for the same question. Every registration is an on-chain transaction, test locally first, every time.

## Register

Easiest path: host the `.wasm` file at a public URL (IPFS or any file host), then submit at `https://integrate.telegraphprotocol.com`, it hashes the file and sends the transaction for you.

Direct path, if wiring into our own tooling:

```solidity
registerWasm(wasmHash, wasmUrl, intent)
```

- `wasmHash`, the keccak256 hash of the exact bytes hosted (the node re-downloads and re-hashes, mismatches are rejected)
- `wasmUrl`, a public `https://` or `ipfs://` URL, ≤32MB
- `intent`, the single canonical intent this module scores for, currently `FRAUD_DETECTION` (see `PROJECT_SPEC.md` Section 4.3)

Costs only gas, no bond, no fee. Deregister anytime with `deregisterEntity(registrationId, 2)`, no penalty, the network falls back to the previous champion or Telegraph's default scorer.

## What happens after registering

`pending` (being evaluated) → `active` (live champion) or `rejected` (with a recorded reason) → possibly `superseded` later by a better module. Check status via the explorer or API using the `registrationId` returned at registration.
