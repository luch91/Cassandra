# Sentinel

Application track entry. Uses real, paid Telegraph miner requests to triage on-chain governance proposals for fraud signals. Full rationale in `../docs/PROJECT_SPEC.md` Section 5, corrected Aug 22 per decision D10.

## The corrected architecture

Sentinel does **not** call DWCS directly. DWCS is a WASM module invoked internally by Telegraph's validators with a `ground_truth` string Sentinel never has access to, there's no channel for an application to invoke a registered scoring module. Instead, Sentinel's confidence signal comes from **querying multiple independent, live miners for the same question and checking whether their answers agree.** Low agreement means low confidence, same underlying "disagreement as signal" idea DWCS uses, just computed over live miner outputs at the application layer instead of inside the WASM sandbox against a known ground truth.

## Build order

1. `src/ingest/governance_source.ts`, pulls real governance proposal text from a chosen source. Needs a concrete decision (which forum/API), not blocked by anything external.
2. `src/scoring/telegraph_client.ts`, real x402-paid requests against Telegraph's Engine. Confirmed and implemented against the real API (`docs.telegraphprotocol.com/docs/using/x402-inference`), needs a funded testnet wallet (`EVM_PRIVATE_KEY` in `.env`, USDC on Base Sepolia).
3. `src/scoring/multi_miner_agreement.ts`, computes agreement across several miners' answers to the same query, fully implemented and tested.
4. `src/onchain/action.ts`, split into two layers. Layer 1 (the x402 payment's own on-chain receipt, `signal_hash`) is automatic and already covered by step 2. Layer 2 (an explicit governance-contract flag write) is a real, still-open decision, see the file's own comments.

## Non-negotiable reminders

- Production code in `src/` must never mock or simulate a Miner response. Test fixtures in `tests/` are fine and clearly labeled.
- `MIN_MINER_SAMPLE_SIZE` (currently 3) exists because sampling only 1 miner makes an agreement score meaningless by construction, don't lower it to make a demo look easier.
- Layer 2's on-chain action must not be implemented against a guessed governance contract interface. Pick the real target first.

## Before running `npm install`

The `@x402/fetch` and `@x402/evm` version numbers in `package.json` are placeholders, not verified against the actual published npm registry versions. Run `npm view @x402/fetch versions` (and the same for `@x402/evm`) and update `package.json` with the real current version before installing. The package names and import paths themselves are confirmed from the docs, only the exact version pins are unverified.
