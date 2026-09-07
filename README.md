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

Its runtime interface returns one `f32` score from `0` to `1`. It has no network access, filesystem access, or persistent state.

### Sentinel

Sentinel is the application layer. It reads active proposals from the public `balancer.eth` Snapshot space, discovers compatible live Telegraph Miners through the public registry, submits real x402 requests, and compares answers across Miners. It records every completed paid request in an append-only local ledger and only records Layer 1 evidence after independent receipt verification succeeds.

High agreement increases confidence. Low agreement indicates that the proposal should receive human review.

The x402 settlement is Sentinel's Layer 1 on-chain evidence. Layer 2, an external governance-contract flag write, is deliberately excluded because Snapshot spaces do not expose a universal, verified flagging interface. Automated polling never reprocesses a proposal after a recorded paid request, preventing duplicate traffic.

## Repository

```text
app/                  Sentinel application
dwcs/rust-module/     Deployable DWCS WASM module
dwcs/src/             TypeScript scoring prototype
dwcs/canaries/        Local held-out adversarial cases
scripts/              Build and validation helpers
```

## Verification and submission status

DWCS is built for `wasm32-unknown-unknown` and validated as a zero-import WASM module. The repository includes Rust and TypeScript tests for its deterministic scoring logic, Snapshot ingestion, and Sentinel's agreement-based triage behavior. A single owner-authorized Sentinel contingency request is documented in GitHub issue #16 with a verified x402 receipt. It used a closed Balancer proposal because no active proposal was available, so it does not prove active-vote production behavior.

DWCS registration is a separate owner-authorized on-chain action and is not claimed as complete by this repository. Track 2 submission status is managed by the project owner.

## Sentinel usage metrics

Usage is measured only from completed, attributable records in the append-only request ledger. The metrics command never sends requests and never fabricates activity:

```bash
npm run build
npm run metrics:sentinel
```

The output reports completed requests, distinct proposal IDs, total cost, a target of 100 real requests, and whether that target has been reached. The current checkout's local ledger reports `0` completed requests, `0` unique proposals, `$0` cost, and `targetReached: false`. The separately documented one-request contingency smoke test is receipt evidence, not volume generation, and is not inserted into the ledger retroactively.

The continuous runner enforces `SENTINEL_MAX_REQUESTS=100` and `SENTINEL_MAX_BUDGET_USD=1` before every paid Miner call. It also reserves `SENTINEL_MAX_REQUEST_COST_USD=0.01` per call by default, and stops permanently when either ceiling would be crossed. If Snapshot has no eligible active proposal, the cycle makes no payment.

## No-UI operation

Cassandra has no web UI. The supported operator surface is the CLI and the JSONL evidence files it produces. Use `npm run preflight:sentinel` for a free registry check, `npm run metrics:sentinel` for usage reporting, and the continuously running `npm run start:sentinel` only when a funded wallet and explicit paid-request authorization are available. Keep `data/` and `.sentinel-evidence/` local and redact secrets before sharing evidence.

## Submission checklist

- [x] Sentinel code, receipt verification, proposal ingestion, retry guard, and ledger metrics are implemented and locally tested.
- [x] One authorized contingency receipt is documented and independently verified in GitHub issue #16.
- [x] The local usage metric is reproducible and non-gamed. Current result: `0/100` real requests.
- [ ] DWCS registration and live status are confirmed on Telegraph.
- [ ] The 100-real-request guardrail is reached and evidenced. Synthetic traffic is prohibited.
- [ ] Final demo and application write-up are supplied by the project owner.

The unchecked items are external or authorization-gated deliverables. They are intentionally not represented as complete by this repository.

## Safety boundaries

- Sentinel production paths use real Telegraph endpoints only.
- DWCS performs deterministic computation over its input strings only.
- Canary data, private keys, wallet files, and payment material are excluded from version control.
