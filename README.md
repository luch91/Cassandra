# Cassandra

Cassandra is a governance-risk pipeline built on Telegraph Protocol.

It helps reviewers identify proposals that need closer scrutiny by combining two distinct forms of signal quality:

- **DWCS** evaluates how well a Telegraph Miner answer matches the available ground truth.
- **Sentinel** asks several independent Miners to assess the same proposal, then uses their agreement as a confidence signal.

Telegraph provides the intelligence network and payment settlement. Cassandra provides a focused governance-review workflow on top of it.

## Components

### DWCS

Disagreement-Weighted Canonical Scoring is a standalone WASM scoring module for `FRAUD_DETECTION`.

It combines normalized word overlap, stopword-weighted overlap, bigram Jaccard similarity, and longest-common-subsequence ratio. When those metrics disagree, the final score is dampened to reduce the benefit of keyword stuffing or other shallow answer imitation.

The deployed interface returns one `f32` score from `0` to `1`. It has no network access, filesystem access, or persistent state.

### Sentinel

Sentinel is the application layer. It sends a governance proposal to real Telegraph Miners, pays through x402, verifies the resulting `signal_hash` receipt, and compares answers across Miners.

High agreement increases confidence. Low agreement indicates that the proposal should receive human review.

## Repository

```text
app/                  Sentinel application
dwcs/rust-module/     Deployable DWCS WASM module
dwcs/src/             TypeScript scoring prototype
dwcs/canaries/        Local held-out adversarial cases
scripts/              Build and validation helpers
```

## Validation

DWCS is built for `wasm32-unknown-unknown` and validated as a zero-import WASM module. The repository includes Rust and TypeScript tests for its deterministic scoring logic and Sentinel's agreement-based triage behavior.

## Safety boundaries

- Sentinel production paths use real Telegraph endpoints only.
- DWCS performs deterministic computation over its input strings only.
- Canary data, private keys, wallet files, and payment material are excluded from version control.
