# Sentinel

Sentinel uses real, paid Telegraph Miner requests to triage active Balancer governance proposals for fraud signals.

## The corrected architecture

Sentinel does **not** call DWCS directly. DWCS is a WASM module invoked internally by Telegraph's validators with a `ground_truth` string Sentinel never has access to, there's no channel for an application to invoke a registered scoring module. Instead, Sentinel's confidence signal comes from **querying multiple independent, live miners for the same question and checking whether their answers agree.** Low agreement means low confidence, same underlying "disagreement as signal" idea DWCS uses, just computed over live miner outputs at the application layer instead of inside the WASM sandbox against a known ground truth.

## Governance source

Sentinel reads active proposals from the public `balancer.eth` Snapshot space through Snapshot's GraphQL API. Snapshot provides the proposal title, body, state, and timestamps Sentinel needs. This source is read-only.

Layer 1 is the real on-chain payment receipt produced by each x402 request. Layer 2 is intentionally not implemented: Snapshot spaces do not provide a universal governance-contract function for externally flagging a proposal, and Cassandra will not invent one. An escalation is instead surfaced for human review with its verified Layer 1 receipt. Agreement alone never escalates a proposal: at least two Miners must also return explicit structured fraud-risk signals.

## Non-negotiable reminders

- Production code in `src/` must never mock or simulate a Miner response. Test fixtures in `tests/` are fine and clearly labeled.
- `MIN_MINER_SAMPLE_SIZE` (currently 3) exists because sampling only 1 miner makes an agreement score meaningless by construction, don't lower it to make a demo look easier.
- Layer 2 remains excluded for this Snapshot source. Do not invent a governance-contract flag interface.

## Running Sentinel

Build with `npm run build`, then run continuously with `npm run start:sentinel`.

Required for real operation: set `EVM_PRIVATE_KEY` to a funded Base Sepolia wallet and `SENTINEL_ALLOW_PAID_REQUESTS=true` for that process. Every inference request is paid and sent only to live Telegraph endpoints. Use a dedicated wallet with a limited balance. A wallet alone cannot trigger spending.

Sentinel reads Telegraph's public integration registry on every cycle and selects only active `FRAUD_DETECTION` miners which declare a POST endpoint accepting a `query` field with no other required input. It does not hardcode a miner, endpoint, or schema.

Configuration defaults are deliberately conservative:

- `SENTINEL_ESCALATION_THRESHOLD=0.85`, accepted range 0.5 to 1.
- `SENTINEL_MIN_REMAINING_VOTE_MINUTES=60`, Sentinel skips proposals inside this pre-vote review buffer.
- `SENTINEL_POLL_INTERVAL_MS=900000`, minimum 60000 ms.
- `SENTINEL_REQUEST_LEDGER=data/sentinel-request-ledger.jsonl`, append-only paid-request attribution ledger.
- `SENTINEL_RECEIPT_LOG=data/sentinel-layer1-receipts.jsonl`, append-only verified receipt evidence.

Sentinel writes an attempt ledger before inference and a request ledger after each completed paid request. This prevents polling from generating duplicate or synthetic traffic, including after a process interruption. Failed attempts require an explicit human decision before retrying. Real request records are gitignored and must be exported explicitly for submission evidence.

Use `npm run preflight:sentinel` after a build to make a free registry-only check that enough compatible miners exist. Use `npm run metrics:sentinel` to report completed attributable requests, distinct proposals, total cost, and progress toward 100 requests. Neither command makes a paid request.

For a bounded receipt diagnostic, set `SENTINEL_SMOKE_QUERY` and
`SENTINEL_ALLOW_PAID_REQUESTS=true`, then run `npm run smoke:sentinel`.
It makes exactly one paid request, performs no retries or loop, and writes only
redacted receipt metadata under `.sentinel-evidence/`.

## Live readiness limitation

The current public Engine OpenAPI documents direct and auto-routed x402 requests, but does not document a `signal_hash` on inference responses or a signal lookup endpoint. Sentinel will fail closed if a paid response lacks `signal_hash`, so it will never label such a response as a verified Layer 1 receipt. Before any live run, perform one explicitly authorized paid smoke test, retain the raw response and receipt evidence, and reconcile the receipt path with Telegraph if the field or lookup route differs.
