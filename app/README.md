# Sentinel

Sentinel uses real, paid Telegraph Miner requests to triage active Balancer governance proposals for fraud signals.

## The corrected architecture

Sentinel does **not** call DWCS directly. DWCS is a WASM module invoked internally by Telegraph's validators with a `ground_truth` string Sentinel never has access to, there's no channel for an application to invoke a registered scoring module. Instead, Sentinel's confidence signal comes from **querying multiple independent, live miners for the same question and checking whether their answers agree.** Low agreement means low confidence, same underlying "disagreement as signal" idea DWCS uses, just computed over live miner outputs at the application layer instead of inside the WASM sandbox against a known ground truth.

## Governance source

Sentinel reads active proposals from the public `balancer.eth` Snapshot space through Snapshot's GraphQL API. Snapshot provides the proposal title, body, state, and timestamps Sentinel needs. This source is read-only.

Layer 1 is the real on-chain payment receipt produced by each x402 request. Layer 2 is intentionally not implemented: Snapshot spaces do not provide a universal governance-contract function for externally flagging a proposal, and Cassandra will not invent one. An escalation is instead surfaced for human review with its verified Layer 1 receipt.

## Non-negotiable reminders

- Production code in `src/` must never mock or simulate a Miner response. Test fixtures in `tests/` are fine and clearly labeled.
- `MIN_MINER_SAMPLE_SIZE` (currently 3) exists because sampling only 1 miner makes an agreement score meaningless by construction, don't lower it to make a demo look easier.
- Layer 2's on-chain action must not be implemented against a guessed governance contract interface. Pick the real target first.

## Running Sentinel

Copy `.env.example` to `.env`, provide a funded Base Sepolia wallet in `EVM_PRIVATE_KEY`, and run the application with your normal TypeScript runner. Each request is paid and sent only to live Telegraph endpoints.
