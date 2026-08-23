# Project Cassandra

Telegraph Protocol program submission. Two deliverables built as one system:

1. **DWCS** (Disagreement-Weighted Canonical Scoring), a Script Author track entry, a real WASM scoring module. See `dwcs/`.
2. **Sentinel**, the Application track entry that pays for real Telegraph miner inference to triage on-chain governance proposals for fraud signals. See `app/`.

The full strategy, rationale, decision log, acceptance criteria and non-negotiables live in `docs/PROJECT_SPEC.md`. That file is the source of truth. Nothing in this codebase should contradict it. If a build decision here conflicts with that document, stop and resolve the conflict before writing more code.

**Architecture was corrected once already, Aug 22, decisions D9 and D10.** The original design assumed DWCS could call external LLM judges and that Sentinel could invoke DWCS directly. Neither is possible: the scoring module runs with no network access, and there's no channel for an application to call a registered scoring module. Both pieces were redesigned around the real, confirmed interfaces. Read `docs/PROJECT_SPEC.md` Section 2 before assuming anything about how the pieces fit together.

## Repo layout

```
telegraph-cassandra/
├── AGENTS.md                     Operating contract for any coding agent working in this repo
├── docs/
│   ├── PROJECT_SPEC.md           Full spec: rationale, decision log, acceptance criteria, non-negotiables
│   ├── OPEN_QUESTIONS.md         Remaining unknowns (most are now resolved, see the file for what's left)
│   ├── TASKS.md                  Working task list against the execution timeline
│   └── PROGRESS_LOG.md           Running build log, doubles as source material for required X updates
├── dwcs/                         Script Author track: the scoring module
│   ├── rust-module/              THE REAL, DEPLOYABLE implementation, compiles to the actual .wasm
│   ├── src/prototype.ts          TS mirror for fast iteration, never the deployed module
│   ├── canaries/                 Held-out adversarial test triples (never exposed to Miners)
│   └── tests/                    Tests for the TS prototype
├── app/                          Application track: Sentinel governance-proposal triage agent
│   ├── src/
│   │   ├── ingest/                Pulls proposal text from the chosen governance source
│   │   ├── scoring/                Real x402 Telegraph client + multi-miner agreement check
│   │   └── onchain/                Layer 1 (automatic payment receipt) + Layer 2 (governance flag, still open)
│   └── tests/
├── scripts/                      Build/test/deploy helper scripts
├── .env.example
└── package.json
```

## Before you write any code

Read `docs/OPEN_QUESTIONS.md` first, most items are now resolved with real answers and citations. The two still genuinely open: item 5 (Miner attribution mechanics, low priority) and item 7 (registration status). Everything else, the WASM interface, the on-chain settlement mechanism, GT status for target intents, is confirmed and the code reflects it.

## Status

`dwcs/rust-module/` and `app/src/scoring/multi_miner_agreement.ts` are real, testable implementations. `app/src/onchain/action.ts` Layer 2 and `app/src/ingest/governance_source.ts` remain stubs pending a document-stream decision, not a documentation gap. See `docs/TASKS.md` for the current task list and `docs/PROGRESS_LOG.md` for what's actually landed.
