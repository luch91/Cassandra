# Project Cassandra, full repository scaffold

This document contains the complete scaffold for the Telegraph Protocol hackathon submission: every file, in full, in the order shown in the tree below.

## Directory tree

```
.env.example
.gitignore
AGENTS.md
AGENT_HANDOVER_PROMPT.md
README.md
app/README.md
app/src/index.ts
app/src/ingest/governance_source.ts
app/src/onchain/action.ts
app/src/scoring/multi_miner_agreement.ts
app/src/scoring/telegraph_client.ts
app/tests/triage.test.ts
docs/OPEN_QUESTIONS.md
docs/PROGRESS_LOG.md
docs/PROJECT_SPEC.md
docs/TASKS.md
dwcs/README.md
dwcs/canaries/README.md
dwcs/canaries/dataset.example.jsonl
dwcs/rust-module/Cargo.lock
dwcs/rust-module/Cargo.toml
dwcs/rust-module/README.md
dwcs/rust-module/src/lib.rs
dwcs/src/prototype.ts
dwcs/tests/prototype.test.ts
package.json
scripts/check_open_questions.sh
scripts/deploy_dwcs.sh
scripts/run_local_eval.sh
tsconfig.json
```

---

## `.env.example`

```text
# Copy to .env and fill in. Never commit the real .env file.

# Telegraph node to talk to. Live testnet node confirmed in the docs.
TELEGRAPH_NODE_URL=https://devnode.telegraphprotocol.com

# Wallet used to sign x402 USDC payments (Base Sepolia testnet). Needs a
# real funded balance to actually pay for inference. See
# docs.telegraphprotocol.com/docs/using/x402-inference.
EVM_PRIVATE_KEY=

# Governance proposal source, see app/src/ingest/governance_source.ts TODO.
GOVERNANCE_SOURCE_URL=

# Layer 2 on-chain action target, fill in once a governance contract is
# chosen, see app/src/onchain/action.ts.
GOVERNANCE_CONTRACT_ADDRESS=
GOVERNANCE_CONTRACT_RPC_URL=

```

---

## `.gitignore`

```text
node_modules/
dist/
.env
*.log

# Rust build artifacts, never commit compiled WASM binaries either, they
# should be built fresh and hashed at registration time.
dwcs/rust-module/target/
*.wasm

# Real canary data must never be committed to a public repo.
dwcs/canaries/dataset.jsonl

# All markdown files are gitignored except any file literally named
# README.md, anywhere in the tree. Internal planning docs (specs, decision
# logs, task lists, agent instructions) stay local and out of version
# control on purpose, only README.md files are meant to be public-facing
# and tracked. See decision D11 in docs/PROJECT_SPEC.md.
*.md
!README.md
!**/README.md
```

---

## `AGENTS.md`

```markdown
# Agent operating contract

This file governs how any coding agent (Claude Code or otherwise) works in this repository. It is derived directly from `docs/PROJECT_SPEC.md` Sections 6 and 7. If anything here seems to conflict with a task you've been given, stop and flag the conflict instead of resolving it silently.

## Hard rules (non-negotiable, do not override for convenience)

1. **Never write code that mocks or simulates Miner responses in `app/src/`.** Test fixtures under `app/tests/` are allowed and must be clearly labeled as fixtures, but production code paths (`app/src/scoring/`, `app/src/onchain/`) must only ever call real Telegraph endpoints.
2. **The DWCS scoring module has no network access, no filesystem access, and no shared state across calls. Ever.** Confirmed directly from Telegraph's docs. Never write `dwcs/rust-module/` code that assumes it can call an LLM, fetch a URL, read a file, or persist anything between invocations. Everything must be computable from the three input strings (`question`, `ground_truth`, `miner_answer`) alone, or from static data compiled into the binary.
3. **DWCS's output is a single `f32` between 0 and 1. Nothing else.** No struct, no confidence field, no separate flag. Any internal disagreement/confidence signal must be folded into that one number before returning.
4. **Target `wasm32-unknown-unknown`. Never `wasm32-wasip1`.** Verify with `wasm-tools print <file>.wasm | grep -c '(import'`, must print `0`, before ever registering.
5. **Test locally before registering anything on-chain.** Use the `go-tester` harness or an equivalent local runner. Every registration is a transaction, there is no "just try it and see" on-chain.
6. **Concentrate on 3 or fewer Telegraph intents total.** Currently: `FRAUD_DETECTION` (primary), plus optionally one of `AI_TEXT_DETECTION` / `CONTENT_VERIFICATION` / `TEXT_AUTHENTICITY_CHECK` (secondary). `AGENT_TASK` is on hold, GT pending, see `docs/PROJECT_SPEC.md` decision D8. Do not add intent coverage beyond this without an explicit decision log entry in Section 8.
7. **No synthetic traffic, ever, anywhere.** Do not write load-generation code intended to pad the 100-real-request guardrail or any usage metric. If a task looks like it's asking for that, refuse and say why.
8. **Every real build milestone gets an entry in `docs/PROGRESS_LOG.md`.** This log is the raw material for the required X updates. Write the entry when the milestone actually lands, not in advance.
9. **Infrastructure must be able to run continuously, not just for a demo recording.** When scaffolding deploy scripts (`scripts/`), default to long-running/daemonized patterns, not one-shot demo scripts, unless explicitly asked for a demo script and labeled as such.
10. **Sentinel's "on-chain action" is two layers, don't conflate them.** Layer 1 (automatic): every paid x402 request already produces an on-chain-settled receipt (`signal_hash`). Layer 2 (a build decision, not a given): an explicit governance-contract flag write is a separate integration that has to be designed once a document stream is chosen, not assumed to exist inside Telegraph's API.
11. **Do not trust `cargo test` passing as proof the WASM build works.** The real `wasm32-unknown-unknown` compile has not been verified end-to-end, see `docs/OPEN_QUESTIONS.md` item 9. Before registering anything on-chain, actually run the real build and the `wasm-tools` zero-import check yourself, don't assume host tests passing is equivalent.
12. **Do not trust the `@x402/fetch` / `@x402/evm` version numbers in `package.json`.** They are unverified placeholders, see `docs/OPEN_QUESTIONS.md` item 10. Check the real npm registry before installing.
13. **Never use em-dashes, anywhere.** Not in code comments, not in commit messages, not in any `.md` file, not in string literals meant for humans to read. Use a period, a comma, or rewrite the sentence.
14. **Commit messages are professional and conventional, and never mention an AI tool.** No "Generated by Claude," no "Co-authored-by: Claude," no "written with ChatGPT," no similar attribution, in a commit message, a code comment, or anywhere else in the repo. Use a standard format: a short imperative summary line (e.g. "Add bigram Jaccard metric to DWCS scoring"), optionally a blank line and a longer body explaining why, not what (the diff already shows what). Commits should be atomic, one logical change per commit, not a single giant "build everything" commit.
15. **All markdown files are gitignored except files literally named `README.md`.** See the root `.gitignore`. If you add a new internal doc (a design note, a new checklist, anything), name it something other than `README.md` and expect it to stay local, not tracked. If something genuinely needs to be public-facing and tracked, it should be a `README.md`, that's the signal, not an accident.

## Before touching specific files

- `dwcs/rust-module/`: this is the real, deployable scoring module. Read `docs/PROJECT_SPEC.md` Section 4.2 before touching it, the mechanism (multi-metric deterministic ensemble) is specific and was corrected once already (decision D9), don't reintroduce the old LLM-judge design. Before ever registering: read `docs/OPEN_QUESTIONS.md` item 9, the real WASM build is not yet verified.
- `dwcs/src/*.ts`: these are prototyping/research utilities only, for exploring and comparing similarity-metric ideas in a friendlier language before porting logic to Rust. They are never compiled to the deployed module. Do not treat them as the real interface.
- `app/src/onchain/*`: read `docs/PROJECT_SPEC.md` Section 5.1 (Layer 1 vs Layer 2) before writing anything here. Layer 1 (x402 payment/signal_hash) is confirmed and buildable now. Layer 2 (governance-contract flag) depends on a document-stream decision that may still be open, check `docs/TASKS.md`.
- `app/src/scoring/telegraph_client.ts` and `package.json`: before running `npm install` or writing more code against `@x402/fetch`/`@x402/evm`, read `docs/OPEN_QUESTIONS.md` item 10, the version pins are unverified placeholders.
- `dwcs/canaries/`: never let contents of this folder be referenced from anything Miners or the public network can see. It's the held-out gaming-resistance test set. Treat it like a secret.
- `docs/PROJECT_SPEC.md`: treat as read-mostly. If a build decision requires changing something in Sections 2-6, that's a decision-log-worthy event, add a row to Section 8 rather than silently editing the rationale.

## Known open gaps, check `docs/OPEN_QUESTIONS.md` for the full detail

As of the last update, items 9 and 10 are real, unresolved gaps in this codebase, not just documentation caveats:
- Item 9: the real `wasm32-unknown-unknown` build has never been successfully verified, only host-side `cargo test`.
- Item 10: `@x402/fetch`/`@x402/evm` version numbers are placeholders.

Both are pass/fail checks you can just run. Do them before treating either piece as "done."

## Definition of done, per acceptance criteria

Do not mark a task complete unless it satisfies the corresponding checklist item in `docs/PROJECT_SPEC.md` Section 7 (7.1 for DWCS, 7.2 for the application, 7.3 cross-cutting). When you finish a checklist-relevant piece of work, update the checkbox in that file directly.

## When in doubt

Prefer stopping and asking over shipping a guess. The spec's own framing: "we cannot afford to make mistakes." A blocked task with a clear question is a better outcome than a shipped assumption that has to be unwound later.
```

---

## `AGENT_HANDOVER_PROMPT.md`

```markdown
# Handover prompt: Project Cassandra (Telegraph Protocol hackathon)

Paste this entire document as your first message to the coding agent picking up this repository. It is self-contained. Do not summarize or skip sections when handing it off.

---

## 1. What you are building

Two linked hackathon submissions for the Telegraph Protocol hackathon, built by one team, under one repository.

**DWCS (Disagreement-Weighted Canonical Scoring).** A real, deployable Telegraph scoring module. It is a sandboxed WASM binary that scores a Miner's answer against a ground truth answer for the `FRAUD_DETECTION` intent (and possibly a second Tier 3 intent). It receives three plain text strings, `question`, `ground_truth`, `miner_answer`, and returns a single float between 0 and 1. It has no network access, no filesystem access, and no shared state between calls. Instead of a single naive similarity check, it computes four structurally different deterministic similarity metrics and combines them, treating disagreement across those metrics as a signal that an answer is gaming one specific metric rather than being genuinely correct.

**Sentinel.** An application that pays for real Telegraph miner inference (via the x402 payment protocol) to triage on-chain governance proposals for fraud signals. Sentinel cannot call DWCS directly, there is no channel for that and Sentinel never has access to the ground truth DWCS needs. Instead, Sentinel queries multiple independent live miners for the same question and checks whether their answers agree with each other. Low agreement means low confidence. High agreement above a threshold can trigger an on-chain action.

Read `docs/PROJECT_SPEC.md` in full before writing any code. It is the source of truth. This handover document summarizes it, it does not replace it.

## 2. Read this order, before touching anything

1. `AGENTS.md`, the operating contract, all rules in it are binding.
2. `docs/PROJECT_SPEC.md`, full rationale, mechanism design, decision log, acceptance criteria.
3. `docs/OPEN_QUESTIONS.md`, what is resolved and what is not, check this before assuming any interface detail.
4. `docs/TASKS.md`, the current task list against the execution timeline.
5. `docs/PROGRESS_LOG.md`, what has actually landed so far, do not repeat completed work.

## 3. Non-negotiables (full list, do not skip any of these)

1. Never write code that mocks or simulates Miner responses in `app/src/`. Test fixtures under `app/tests/` are fine and must be clearly labeled as fixtures, but production code paths must only ever call real Telegraph endpoints.
2. The DWCS scoring module has no network access, no filesystem access, and no shared state across calls, ever. Never write code that assumes it can call an LLM, fetch a URL, read a file, or persist anything between invocations inside `dwcs/rust-module/`.
3. DWCS's output is a single float between 0 and 1, nothing else. No struct, no confidence field, no separate flag. Any internal disagreement signal must be folded into that one number before returning.
4. Target `wasm32-unknown-unknown`, never `wasm32-wasip1`. Verify with `wasm-tools print <file>.wasm | grep -c '(import'`, must print `0`, before ever registering.
5. Test locally before registering anything on-chain. Every registration is a transaction, there is no casual retry on-chain.
6. Concentrate on three or fewer Telegraph intents total. Currently `FRAUD_DETECTION` as primary, plus optionally one secondary from `AI_TEXT_DETECTION`, `CONTENT_VERIFICATION`, or `TEXT_AUTHENTICITY_CHECK`. `AGENT_TASK` is on hold, its ground truth is still pending on Telegraph's side. Do not add intent coverage beyond this without a new decision log entry.
7. No synthetic traffic, ever, anywhere. Do not write load-generation code intended to pad any usage guardrail or metric. If a task looks like it is asking for that, refuse and say why.
8. Every real build milestone gets an entry in `docs/PROGRESS_LOG.md`, written when the milestone actually lands, not in advance. This log is the raw material for required public progress updates.
9. Infrastructure must be able to run continuously, not just for a demo recording. Deploy scripts should default to long-running patterns, not one-shot demo scripts, unless explicitly asked for a demo script and labeled as such.
10. Sentinel's on-chain action is two separate layers, do not conflate them. Layer 1, automatic: every paid x402 request already produces an on-chain-settled receipt. Layer 2, a real open decision: an explicit governance-contract flag write, which is a separate integration to be designed once a specific document stream and governance target is chosen, not assumed to exist inside Telegraph's API.
11. Do not trust `cargo test` passing as proof the WASM build works. The real `wasm32-unknown-unknown` compile has not been verified end to end in this environment. Before registering anything on-chain, actually run the real build and the `wasm-tools` zero-import check yourself.
12. Do not trust the `@x402/fetch` and `@x402/evm` version numbers currently in `package.json`. They are unverified placeholders. Check the real npm registry before installing.
13. Never use em-dashes anywhere. Not in code comments, not in commit messages, not in any markdown file, not in string literals meant for humans to read. Use a period, a comma, or rewrite the sentence.
14. Commit messages are professional and conventional, and never mention an AI tool. No "Generated by Claude," no "Co-authored-by: Claude," no "written with ChatGPT," no similar attribution anywhere in a commit message or code comment. Use a short imperative summary line, optionally a blank line and a body explaining why, not what. Commits are atomic, one logical change per commit.
15. All markdown files are gitignored except files literally named `README.md`. If you add a new internal doc, expect it to stay local and untracked unless you deliberately name it `README.md` because it is meant to be public-facing.

## 4. Full decision log

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Skip the Miner Track entirely | Wrapping an API is the crowded, undefensible default. The team's edge is in consensus and scoring mechanisms, not API integration. |
| D2 | Originally targeted `FRAUD_DETECTION` and `AGENT_TASK` first | Both had no ground truth methodology defined yet, first mover advantage. Superseded by D8. |
| D3 | Disagreement is a signal about Miner output, never license for the script's own volatility | The network discards scripts that drift from consensus. Conflating detection of gaming with the script's own instability would get the script discarded by the mechanism it is trying to exploit. |
| D4 | Build the script and the application together, single team, single narrative | Eligibility guardrails require active miners and real requests per intent. Owning both ends generates qualifying demand deterministically instead of hoping strangers do it. |
| D5 | Chose on-chain governance proposals as Sentinel's document stream | Tightest demoable loop, closest match to prior shipped work, easiest to source real documents without private data partnerships. |
| D6 | Concentrate on three or fewer intents total | Guardrail math directly punishes spreading thin across many intents. |
| D7 | Did not finalize the WASM interface by guessing | The docs site did not yield extractable technical content via automated fetch at the time, guessing a schema risked wasted build days. Resolved once real docs were fetched, see D9. |
| D8 | Made `FRAUD_DETECTION` the sole primary intent, moved `AGENT_TASK` to monitor only | Telegraph team confirmed directly that `FRAUD_DETECTION`'s ground truth is finalized and scoreable now, `AGENT_TASK`'s is still pending on their side. |
| D9 | Pivoted DWCS from a multi-LLM-judge ensemble to a deterministic multi-metric ensemble | Telegraph's docs confirmed the scoring module has no network access at all, an LLM judge cannot be called from inside it. The disagreement-as-signal idea survived the pivot, computed across diverse deterministic similarity metrics instead of diverse LLM judge passes. |
| D10 | Split Sentinel's on-chain action into Layer 1, automatic payment settlement, and Layer 2, an optional governance-contract flag | Telegraph's own settlement is the x402 payment receipt, it does not include any DAO-specific action. Keeping these as two layers means Layer 1 alone already satisfies the "must use Telegraph miners" requirement with a real receipt, and Layer 2 remains a scoped, explicit decision rather than an assumed given. |
| D11 | Adopted strict repository hygiene, no em-dashes, professional commit messages with no AI-tool attribution, gitignore every markdown file except `README.md` | This is a hackathon submission that judges and other builders will actually read and clone. A clean, professional repository is part of the credibility argument alongside the technical one. |

## 5. Current status, what is real versus what is a stub

**Implemented and tested (real, not a stub):**
- `dwcs/rust-module/src/lib.rs`, the actual scoring module. Four metrics (word overlap, stopword weighted overlap, bigram Jaccard, longest common subsequence ratio), variance based damping into a single float. Eight unit tests, all passing on `cargo test` (host build).
- `dwcs/src/prototype.ts`, a TypeScript mirror of the same logic for fast iteration, with its own passing test suite. This is never the deployed module.
- `app/src/scoring/telegraph_client.ts`, real x402 payment flow client (discovery, paid ask requests, signal verification).
- `app/src/scoring/multi_miner_agreement.ts`, the real agreement based confidence and triage logic, tested.
- `app/src/onchain/action.ts`, Layer 1 (collecting real payment receipts) is implemented. Layer 2 is an intentional stub, see below.

**Intentional stubs, blocked on real decisions, not documentation gaps:**
- `app/src/onchain/action.ts`'s `executeLayer2GovernanceFlag`, blocked on choosing a specific governance contract target once the document stream is finalized.
- `app/src/ingest/governance_source.ts`, blocked on choosing a specific real governance data source.

**Verification gaps, real and unresolved, check `docs/OPEN_QUESTIONS.md` items 9 and 10 for full detail:**
- The real `wasm32-unknown-unknown` build has never been confirmed to succeed, only the host `cargo test` build has been run.
- The `@x402/fetch` and `@x402/evm` version numbers in `package.json` are unverified placeholders.

## 6. Acceptance criteria, summarized (full detail in `PROJECT_SPEC.md` Section 7)

**Script Author track (DWCS):**
- Compiles to a real WASM binary exporting `alloc`, `dealloc`, `rank_answer`, targeting `wasm32-unknown-unknown`, verified zero imports.
- Passes structural checks: loads correctly, blank answer scores exactly zero, correct answer beats unrelated answer, handles adversarial input without crashing.
- Passes a locally replicated benchmark: self-match on a perfect answer at or above 0.75, real variance across a benchmark set, consistent margin between good and bad answers.
- A documented canary case shows naive word overlap would have failed where DWCS's combined metric did not.
- Registered and live scoring `FRAUD_DETECTION` before the Track 1 and 2 deadline.
- At least three substantive, tagged progress updates posted publicly before submission.

**Application track (Sentinel):**
- Makes real, paid requests to real live Telegraph miners, verified against actual responses and receipts.
- Demoable end to end flow: proposal input, paid request, miner responses, agreement based triage decision, on-chain receipt, and Layer 2 action if implemented.
- At least 100 real requests generated against the target intent before the deadline.
- Configurable, documented confidence threshold with a written rationale.
- A clear, non-gamed usage metric.

**Cross cutting:**
- Both submissions reference each other in their write ups.
- Nothing in either submission uses simulated Miner data anywhere, including early testing artifacts.

## 7. Open items, ranked by what actually blocks progress

1. Verify the real `wasm32-unknown-unknown` build succeeds and passes the zero-import check. This blocks any on-chain registration.
2. Verify the real `@x402/fetch` and `@x402/evm` npm versions. This blocks a clean `npm install` for the application.
3. Decide the secondary Tier 3 intent (`AI_TEXT_DETECTION`, `CONTENT_VERIFICATION`, or `TEXT_AUTHENTICITY_CHECK`). Registration is confirmed open for all three, this is a choice, not a blocker to research further.
4. Decide the specific governance document source and target contract for Sentinel. This unblocks both `governance_source.ts` and Layer 2 of the on-chain action.
5. Confirm hackathon registration status for the team.
6. Populate the real canary dataset in `dwcs/canaries/dataset.jsonl` with reviewed adversarial examples, replacing the placeholder content in `dataset.example.jsonl`.
7. Miner Track attribution mechanics, low priority, only relevant if a thin Miner is ever added later.

## 8. Immediate next actions, in order

1. Run `cd dwcs/rust-module && cargo test` to confirm the current baseline still passes.
2. Run `rustup target add wasm32-unknown-unknown && cargo build --release --target wasm32-unknown-unknown` and the `wasm-tools` zero-import check. Report the actual result, do not assume success.
3. Check real npm registry versions for `@x402/fetch` and `@x402/evm`, update `package.json`.
4. Work through `docs/TASKS.md` in order from the top, checking items off as they land for real and logging milestones in `docs/PROGRESS_LOG.md`.

## 9. Final operating principle

When in doubt, stop and ask rather than guess. A blocked task with a clear, specific question is a better outcome than a shipped assumption that has to be unwound later. This project explicitly cannot afford avoidable mistakes given the hackathon deadline. Treat every interface, every contract, and every dependency version as unverified until you have actually checked it yourself, not assumed it from a prior description, including this one.
```

---

## `README.md`

```markdown
# Project Cassandra

Telegraph Protocol hackathon submission. Two deliverables built as one system:

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
```

---

## `app/README.md`

```markdown
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
```

---

## `app/src/index.ts`

```typescript
/**
 * Sentinel entry point, corrected Aug 22. Wires ingestion -> multi-miner
 * ask -> agreement-based triage -> Layer 1 receipt collection -> Layer 2
 * governance flag (once that decision is made).
 */

import { fetchPendingProposals } from "./ingest/governance_source";
import { askMultipleMiners } from "./scoring/telegraph_client";
import { computeAgreement, decideTriageAction, MIN_MINER_SAMPLE_SIZE } from "./scoring/multi_miner_agreement";
import { collectLayer1Receipts, executeLayer2GovernanceFlag } from "./onchain/action";

export async function runSentinelCycle(source: string): Promise<void> {
  const proposals = await fetchPendingProposals(source);

  for (const proposal of proposals) {
    const query = `Does this governance proposal show signs of fraud or fabricated evidence? Proposal: ${proposal.title}\n\n${proposal.bodyText}`;

    const askResults = await askMultipleMiners("FRAUD_DETECTION", query, MIN_MINER_SAMPLE_SIZE);

    const agreement = computeAgreement(
      askResults.map((r) => ({ minerId: r.miner_id, answerText: JSON.stringify(r.result) }))
    );

    const decision = decideTriageAction(agreement);

    // Layer 1 always happens, it's just collecting what already occurred.
    const receipt = collectLayer1Receipts(askResults);
    console.log(`Proposal ${proposal.id}: ${decision.action}, ${decision.reason}`, receipt);

    if (decision.action === "escalate_onchain") {
      // Layer 2 is still an open decision, see onchain/action.ts. This
      // call will throw until a governance target is chosen and
      // implemented against its real contract interface.
      await executeLayer2GovernanceFlag(proposal, decision);
    }
  }
}
```

---

## `app/src/ingest/governance_source.ts`

```typescript
/**
 * Governance proposal ingestion for Sentinel.
 * See PROJECT_SPEC.md Section 5.2.
 *
 * TODO: pick a concrete public governance source (a specific DAO forum,
 * Snapshot space, or on-chain governance contract) and implement a real
 * fetcher here. Not blocked by open questions, this just needs a decision.
 * Record that decision as a new row in PROJECT_SPEC.md Section 8 once made,
 * it's a real project decision, not an implementation detail to bury silently.
 */

export interface GovernanceProposal {
  id: string;
  source: string;
  title: string;
  bodyText: string;
  linkedEvidenceUrls: string[];
  submittedAt: string; // ISO 8601
}

export async function fetchPendingProposals(_source: string): Promise<GovernanceProposal[]> {
  throw new Error(
    "Not implemented: pick a concrete governance source and implement a real " +
    "fetcher. See file header comment."
  );
}
```

---

## `app/src/onchain/action.ts`

```typescript
/**
 * Sentinel's on-chain action, corrected Aug 22 per decision D10.
 *
 * Two distinct layers, do not conflate them:
 *
 * Layer 1 (confirmed, automatic): every paid x402 request Sentinel makes
 * via telegraph_client.ts already produces an on-chain-settled payment and
 * a signal_hash receipt, independently verifiable and visible on
 * explorer.telegraphprotocol.com. This alone satisfies "must use Telegraph
 * miners" with a real on-chain artifact. No extra code needed beyond what
 * telegraph_client.ts already does.
 *
 * Layer 2 (our own addition, still an open decision): an explicit "flag
 * this governance proposal" write to whatever DAO/governance contract the
 * chosen document stream (PROJECT_SPEC.md Section 5.2) actually uses.
 * Telegraph does not provide this, it is not a documentation gap, it's a
 * real scope item that depends on picking a specific governance target.
 * Do not implement this against a guessed contract interface.
 */

import type { TriageDecision } from "../scoring/multi_miner_agreement";
import type { GovernanceProposal } from "../ingest/governance_source";
import type { AskResult } from "../scoring/telegraph_client";

export interface Layer1Receipt {
  signalHash: string;
  verifiedAt: string;
  minerIds: string[];
}

/**
 * Layer 1: just collects the receipts Sentinel already has from its paid
 * requests. This is real and buildable now, it doesn't need anything new.
 */
export function collectLayer1Receipts(askResults: AskResult[]): Layer1Receipt {
  return {
    signalHash: askResults[0]?.signal_hash ?? "",
    verifiedAt: new Date().toISOString(),
    minerIds: askResults.map((r) => r.miner_id),
  };
}

export interface Layer2ActionResult {
  txHash: string;
  action: string;
  confirmedAt: string;
}

/**
 * Layer 2: NOT IMPLEMENTED. Blocked on choosing a specific governance
 * contract/interface once the document stream (PROJECT_SPEC.md Section
 * 5.2) is finalized. This is a real product decision, not something to
 * guess your way past, different DAOs expose completely different
 * governance contract shapes (Governor Bravo-style, Snapshot + a custom
 * execution module, a bespoke contract, etc).
 */
export async function executeLayer2GovernanceFlag(
  _proposal: GovernanceProposal,
  _decision: TriageDecision
): Promise<Layer2ActionResult> {
  throw new Error(
    "Not implemented: Layer 2 governance-contract flag action depends on " +
    "which specific governance target Sentinel is built against. Decide " +
    "the document stream/target contract first (PROJECT_SPEC.md Section " +
    "5.2), then implement against that contract's real interface, never " +
    "a guessed one."
  );
}
```

---

## `app/src/scoring/multi_miner_agreement.ts`

```typescript
/**
 * Multi-Miner agreement check for Sentinel.
 *
 * CORRECTED Aug 22: this file originally assumed Sentinel could call DWCS
 * directly to get a confidence score. That's not possible, DWCS is a WASM
 * module invoked internally by Telegraph's validators with a ground_truth
 * string Sentinel never has access to (that's the entire point of asking
 * Telegraph in the first place). There is no channel for an application to
 * invoke a registered scoring module directly.
 *
 * Sentinel's actual confidence signal instead comes from querying multiple
 * independent, live FRAUD_DETECTION miners for the *same* proposal (via
 * GET /api/miners?intent=FRAUD_DETECTION to discover them, then paying each
 * via x402, see telegraph_client.ts) and checking whether their answers
 * agree. This is the same "disagreement as measurement instrument"
 * philosophy DWCS uses, just applied at the application layer over live
 * miner outputs instead of inside the WASM sandbox over a known ground
 * truth. Low agreement across miners means low confidence, same as DWCS's
 * internal variance check, just computed over different inputs.
 */

export interface MinerAnswer {
  minerId: string;
  answerText: string;
}

export interface AgreementResult {
  agreementScore: number; // 0-1, how much the sampled miners agree with each other
  sampleSize: number;
  representativeAnswer: string; // the most-agreed-with answer, used downstream
}

/**
 * Cheap agreement check: pairwise word-overlap similarity between every
 * pair of miner answers, averaged. This deliberately reuses the same kind
 * of lightweight, deterministic similarity approach as DWCS's metrics
 * (see dwcs/rust-module/src/lib.rs), applied miner-answer-to-miner-answer
 * instead of answer-to-ground-truth. Good enough for an app-layer
 * confidence signal, this does not need to be as rigorous as the actual
 * scoring module since it's not what determines Miner payouts.
 */
function wordOverlap(a: string, b: string): number {
  const aWords = a.toLowerCase().split(/\s+/).filter(Boolean);
  const bWords = new Set(b.toLowerCase().split(/\s+/).filter(Boolean));
  if (aWords.length === 0) return 0;
  const matched = aWords.filter((w) => bWords.has(w)).length;
  return matched / aWords.length;
}

export function computeAgreement(answers: MinerAnswer[]): AgreementResult {
  if (answers.length === 0) {
    return { agreementScore: 0, sampleSize: 0, representativeAnswer: "" };
  }
  if (answers.length === 1) {
    return { agreementScore: 1, sampleSize: 1, representativeAnswer: answers[0].answerText };
  }

  let totalScore = 0;
  let pairCount = 0;
  const perAnswerAvgAgreement: number[] = answers.map(() => 0);

  for (let i = 0; i < answers.length; i++) {
    for (let j = 0; j < answers.length; j++) {
      if (i === j) continue;
      const sim = wordOverlap(answers[i].answerText, answers[j].answerText);
      totalScore += sim;
      pairCount += 1;
      perAnswerAvgAgreement[i] += sim;
    }
  }

  const agreementScore = pairCount === 0 ? 0 : totalScore / pairCount;

  // the "representative" answer is whichever answer agreed most with the
  // rest of the sample, a cheap stand-in for a real consensus mechanism.
  let bestIdx = 0;
  for (let i = 1; i < answers.length; i++) {
    if (perAnswerAvgAgreement[i] > perAnswerAvgAgreement[bestIdx]) bestIdx = i;
  }

  return {
    agreementScore,
    sampleSize: answers.length,
    representativeAnswer: answers[bestIdx].answerText,
  };
}

export interface TriageDecision {
  action: "flag_for_review" | "escalate_onchain" | "no_action";
  reason: string;
}

/**
 * Minimum number of independent miners to sample per proposal before
 * trusting an agreement score at all. Sampling only 1 miner gives an
 * agreementScore of 1 by construction, which would be meaningless as a
 * confidence signal, PROJECT_SPEC.md acceptance criteria should require
 * at least 3 here once finalized.
 */
export const MIN_MINER_SAMPLE_SIZE = 3;

export const DEFAULT_ESCALATION_THRESHOLD = 0.85;

export function decideTriageAction(
  agreement: AgreementResult,
  escalationThreshold: number = DEFAULT_ESCALATION_THRESHOLD
): TriageDecision {
  if (agreement.sampleSize < MIN_MINER_SAMPLE_SIZE) {
    return {
      action: "flag_for_review",
      reason: `Only sampled ${agreement.sampleSize} miner(s), below the minimum of ${MIN_MINER_SAMPLE_SIZE} needed to trust an agreement score.`,
    };
  }

  if (agreement.agreementScore < 0.5) {
    return {
      action: "flag_for_review",
      reason: `Miners disagreed significantly (agreement ${agreement.agreementScore.toFixed(2)}), needs human review before any on-chain action.`,
    };
  }

  if (agreement.agreementScore >= escalationThreshold) {
    return {
      action: "escalate_onchain",
      reason: `High miner agreement (${agreement.agreementScore.toFixed(2)}) cleared the escalation threshold (${escalationThreshold}).`,
    };
  }

  return {
    action: "no_action",
    reason: `Miner agreement (${agreement.agreementScore.toFixed(2)}) did not clear the escalation threshold (${escalationThreshold}).`,
  };
}

```

---

## `app/src/scoring/telegraph_client.ts`

```typescript
/**
 * Real Telegraph Engine client for Sentinel, using the confirmed x402
 * payment flow. Source: docs.telegraphprotocol.com/docs/using/x402-inference
 *
 * Flow: discover live miners for an intent -> POST an ask request -> get
 * an HTTP 402 challenge back -> sign a USDC payment -> retry with the
 * payment header -> receive the miner's answer plus a signal_hash receipt.
 *
 * This is Layer 1 of Sentinel's "on-chain action" (see PROJECT_SPEC.md
 * Section 5.1, decision D10): the payment itself is the on-chain-settled
 * receipt, verifiable independently via GET /engine/v1/signal/{signal_hash}.
 * Layer 2 (a DAO-specific "flag this proposal" write) is a separate
 * concern, see onchain/action.ts.
 */

import { wrapFetchWithPayment } from "@x402/fetch";
import { createSigner } from "@x402/evm";

export type TelegraphIntentId =
  | "FRAUD_DETECTION"
  | "CONTENT_VERIFICATION"
  | "AI_TEXT_DETECTION"
  | "AGENT_TASK";

export interface MinerCatalogEntry {
  id: string;
  name: string;
  intents: TelegraphIntentId[];
  min_price_usdc: number;
  status: string;
}

export interface AskResult {
  miner_id: string;
  miner_name: string;
  result: unknown; // shape depends on the specific miner's declared output schema
  cost_usd: number;
  duration_ms: number;
  signal_hash: string;
}

const TELEGRAPH_NODE_URL = process.env.TELEGRAPH_NODE_URL ?? "https://devnode.telegraphprotocol.com";

function getFetchWithPayment() {
  const privateKey = process.env.EVM_PRIVATE_KEY;
  if (!privateKey) {
    throw new Error(
      "EVM_PRIVATE_KEY is not set. Sentinel needs a funded testnet wallet " +
      "(USDC on Base Sepolia) to pay for x402 requests. See .env.example."
    );
  }
  const signer = createSigner(privateKey);
  return wrapFetchWithPayment(fetch, signer);
}

/**
 * GET /api/miners?intent=... Discovery endpoint, no payment required.
 * Always call this fresh rather than caching a hardcoded miner list, per
 * the docs: "the set of live miners changes as operators register and
 * deregister on-chain, treat this endpoint as the source of truth."
 */
export async function discoverMiners(intent: TelegraphIntentId): Promise<MinerCatalogEntry[]> {
  const res = await fetch(`${TELEGRAPH_NODE_URL}/api/miners?intent=${intent}&status=active`);
  if (!res.ok) {
    throw new Error(`Failed to discover miners for ${intent}: HTTP ${res.status}`);
  }
  return res.json();
}

/**
 * Pays for and executes a single ask request against one miner. Handles
 * the full 402 challenge/payment/retry cycle via @x402/fetch.
 *
 * Non-negotiable: this must only ever call the real Telegraph endpoint.
 * Never fabricate a response here, if the call fails, let it throw.
 */
export async function askMiner(minerId: string, query: string): Promise<AskResult> {
  const fetchWithPayment = getFetchWithPayment();
  const res = await fetchWithPayment(`${TELEGRAPH_NODE_URL}/engine/v1/ask/${minerId}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query }),
  });

  if (!res.ok) {
    throw new Error(`Ask request to miner ${minerId} failed: HTTP ${res.status}`);
  }

  return res.json();
}

/**
 * Independently verifies a call after the fact via its signal_hash. This
 * is how Layer 1's "on-chain receipt" gets checked, not trusting the
 * original response alone.
 */
export async function verifySignal(signalHash: string): Promise<unknown> {
  const res = await fetch(`${TELEGRAPH_NODE_URL}/engine/v1/signal/${signalHash}`);
  if (!res.ok) {
    throw new Error(`Failed to verify signal ${signalHash}: HTTP ${res.status}`);
  }
  return res.json();
}

/**
 * Queries N distinct live miners for the same intent and query, used by
 * multi_miner_agreement.ts to compute an app-layer confidence signal.
 * See that file's header comment for why this replaces calling DWCS
 * directly, which isn't possible from application code.
 */
export async function askMultipleMiners(
  intent: TelegraphIntentId,
  query: string,
  sampleSize: number
): Promise<AskResult[]> {
  const miners = await discoverMiners(intent);
  if (miners.length < sampleSize) {
    throw new Error(
      `Only ${miners.length} live miner(s) available for ${intent}, need at least ${sampleSize}. ` +
      `This also means the intent likely doesn't clear the 3-active-Miner guardrail yet.`
    );
  }
  const chosen = miners.slice(0, sampleSize);
  return Promise.all(chosen.map((m) => askMiner(m.id, query)));
}
```

---

## `app/tests/triage.test.ts`

```typescript
import {
  computeAgreement,
  decideTriageAction,
  DEFAULT_ESCALATION_THRESHOLD,
  MIN_MINER_SAMPLE_SIZE,
  type MinerAnswer,
} from "../src/scoring/multi_miner_agreement";

describe("computeAgreement", () => {
  it("returns full agreement trivially for a single answer (and callers should not trust this)", () => {
    const answers: MinerAnswer[] = [{ minerId: "1", answerText: "this proposal looks legitimate" }];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBe(1);
    expect(result.sampleSize).toBe(1);
  });

  it("scores high agreement when miners give similar answers", () => {
    const answers: MinerAnswer[] = [
      { minerId: "1", answerText: "this proposal shows signs of fraud and fabricated evidence" },
      { minerId: "2", answerText: "signs of fraud and fabricated evidence are present in this proposal" },
      { minerId: "3", answerText: "fraud and fabricated evidence detected in this proposal" },
    ];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBeGreaterThan(0.5);
  });

  it("scores low agreement when miners disagree", () => {
    const answers: MinerAnswer[] = [
      { minerId: "1", answerText: "this proposal is completely legitimate and well documented" },
      { minerId: "2", answerText: "fraud detected fabricated evidence manipulation" },
      { minerId: "3", answerText: "unable to determine anything from the provided text" },
    ];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBeLessThan(0.5);
  });
});

describe("decideTriageAction", () => {
  it("flags for review when sample size is below the minimum", () => {
    const decision = decideTriageAction({ agreementScore: 0.99, sampleSize: 1, representativeAnswer: "x" });
    expect(decision.action).toBe("flag_for_review");
  });

  it("flags for review when agreement is low even with enough samples", () => {
    const decision = decideTriageAction({
      agreementScore: 0.2,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("flag_for_review");
  });

  it("escalates on-chain when agreement clears the threshold with enough samples", () => {
    const decision = decideTriageAction({
      agreementScore: DEFAULT_ESCALATION_THRESHOLD + 0.05,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("escalate_onchain");
  });

  it("takes no action when agreement is moderate but below threshold", () => {
    const decision = decideTriageAction({
      agreementScore: 0.6,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("no_action");
  });
});
```

---

## `docs/OPEN_QUESTIONS.md`

```markdown
# Open questions

Mirrors `PROJECT_SPEC.md` Section 9. This file exists separately so it's easy for an agent to check quickly before touching gated files. Update both files together when an item is resolved, do not let them drift apart.

Status legend: `BLOCKING` (gates specific files), `RESOLVED` (answer recorded below), `DEPRIORITIZED` (not urgent yet).

---

### 1. WASM script interface/contract [RESOLVED]
Input format (what does a Miner response object look like when it reaches the script?), output format, and the runtime/toolchain expected.
**Answer:** RESOLVED, Aug 22, from `https://docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module`. The module receives exactly three plain-text strings as `(pointer, length)` pairs, in this fixed order: `question`, `ground_truth` (the correct answer, provided directly, not hidden from the scoring module), `miner_answer`. It does not receive JSON, structured fields, confidence values, or API metadata. It exports exactly three functions: `alloc`, `dealloc`, `rank_answer`. `rank_answer` takes six i32 params (ptr+len for each of the three strings) and returns a single `f32` between 0 and 1. Any language that compiles to a standalone WASM binary works (Rust recommended, also C/C++, TinyGo, AssemblyScript), target must be `wasm32-unknown-unknown` (not `wasm32-wasip1`), 32MB size cap, and the module runs in a sandbox with **no network access, no filesystem access, no shared state across calls.** This is the single most important correction to the original design, see decision D9.

### 2. Native confidence/contested field support [RESOLVED]
Whether Telegraph's script output format natively supports a confidence or contested field.
**Answer:** RESOLVED, Aug 22. No. Output is a single `f32`, nothing else. Any internal confidence/disagreement signal DWCS computes must be folded into that one number, never published as a separate field.

### 3. Re-evaluation/re-sampling support [RESOLVED]
Whether a "contested" output can trigger re-sampling within the protocol.
**Answer:** RESOLVED, Aug 22 (confirmed directly by the Telegraph team). No. The module is invoked once per epoch, every score is final, there is no retry and no re-evaluation trigger. Whatever `rank_answer` returns is used for ranking, full stop.

### 4. On-chain settlement/receipt mechanism [RESOLVED]
What the Application track needs to actually call/integrate to produce a real on-chain action tied to a Telegraph response.
**Answer:** RESOLVED, Aug 22, from `https://docs.telegraphprotocol.com/docs/using/x402-inference` and the team's direct reply. Applications pay per call via the x402 HTTP payment standard: request, receive an HTTP 402 challenge, sign a USDC payment (Base Sepolia or Solana Devnet) via an x402 client library (`@x402/fetch`, `@x402/evm`), retry with the `PAYMENT-SIGNATURE` header. The response includes the miner's result plus a `signal_hash`, verifiable afterward via `GET /engine/v1/signal/{signal_hash}`. This is the "cryptographic receipt," visible on `https://explorer.telegraphprotocol.com/`. Note: this is Telegraph's own settlement, it proves the paid inference happened and what it returned. It does **not** include any DAO-specific "flag this proposal" action, that remains a separate integration into whatever governance contract Sentinel targets, and is still an open implementation decision (not a Telegraph API gap).

### 5. Miner Track attribution mechanics [DEPRIORITIZED]
How "number of applications built on your Miner" and "total requests served" are measured. Not our track, but affects instrumentation if a thin Miner is ever added later.
**Answer:** not yet resolved, low priority.

### 6. Whether FRAUD_DETECTION / AGENT_TASK are scoreable yet [RESOLVED]
Whether Script Authors can submit scores for intents with no defined ground truth yet, or whether these are non-functional placeholders pending a GT definition from the Telegraph team.
**Answer:** RESOLVED, Aug 21. Telegraph team confirmed:
- `FRAUD_DETECTION`: GT is **finalized**. Scoreable now.
- `AGENT_TASK`: GT is **pending**, still being worked on by the Telegraph team. Not scoreable yet.

**Action taken:** `FRAUD_DETECTION` becomes the sole primary intent for DWCS. `AGENT_TASK` is deprioritized to "monitor and revisit" until its GT lands, do not build against it as a primary target. See `PROJECT_SPEC.md` Section 4.3 (updated) and Section 8 decision log (D8).

### 7. Registration status [BLOCKING: nothing in code, but blocks all live/on-chain testing]
Confirm whether registration for the team is complete. Registration reportedly unlocks early track access, task specs, and a private Discord channel.
**Answer:** not yet resolved.

### 8. GT status of Tier 3 secondary intents [PARTIALLY RESOLVED, NOT a build blocker]
Whether `AI_TEXT_DETECTION`, `CONTENT_VERIFICATION`, and `TEXT_AUTHENTICITY_CHECK` have finalized GT, same question as item 6 but for the secondary intent candidates. Note: this gates live deployment/testing against these specific intents only, it does not block writing or testing `dwcs/src/` locally, which is intent-agnostic until the WASM interface layer.
**Answer:** partially resolved, Aug 22. Telegraph team confirmed registration is open for these intents ("Yes you can register for those"). Registration is an administrative step to participate in an intent's live scoring, it is not a prerequisite for local build work. This is an operational green light but does not individually confirm GT-finalized status per intent the way item 6 was confirmed for `FRAUD_DETECTION`/`AGENT_TASK`. Team also asked which track we're building on (Miner or WASM/Script Author); replied confirming WASM/Script Author track plus a separate Application track submission.

### 9. Real wasm32-unknown-unknown build, NOT YET VERIFIED [BLOCKING, do this before any registration]
Whether `dwcs/rust-module/src/lib.rs` actually compiles to a valid `.wasm` binary for the real target Telegraph requires.
**Answer:** NOT resolved, this is a live gap, not a documentation question. `cargo test` (host, x86_64) passes all 8 unit tests, that was actually run and is real. The real `cargo build --release --target wasm32-unknown-unknown` was attempted in the environment this scaffold was built in and failed with "can't find crate for core", because `rustup` wasn't available there to install the target, not because of a reported logic error in the code. That distinction matters, but it is not the same as a confirmed successful WASM build. **Action required before registering anything on-chain:** run `rustup target add wasm32-unknown-unknown`, then `cargo build --release --target wasm32-unknown-unknown`, then the `wasm-tools` zero-import check (all three commands are in `dwcs/rust-module/README.md`). If the build surfaces a real compile error beyond the missing-target one already seen, fix it there, don't assume the host tests passing is equivalent to a working WASM build.

### 10. `@x402/fetch` / `@x402/evm` exact version pins [minor, not a design blocker]
The version numbers written into `package.json` for these two packages.
**Answer:** NOT verified. The package names and import paths (`wrapFetchWithPayment`, `createSigner`) are confirmed directly from Telegraph's own docs. The specific version numbers in `package.json` are placeholders that were never checked against the real npm registry. Run `npm view @x402/fetch versions` and `npm view @x402/evm versions` and update `package.json` with real current versions before running `npm install`.

---

## How to resolve an item

1. Check the actual rendered docs site in a real browser (it is a JS-rendered SPA, static fetch does not work).
2. If not answered there, ask in the official Telegraph Discord.
3. Update the "Answer" line here with the confirmed fact and a source/date.
4. Update the corresponding status in `PROJECT_SPEC.md` Section 9 to match.
5. Remove the `TODO(open-question-N)` marker from the file(s) it was blocking.
```

---

## `docs/PROGRESS_LOG.md`

```markdown
# Progress log

Real build milestones only, entered when they actually land, not in advance. This is the raw material for the X progress updates required by the non-negotiables in `PROJECT_SPEC.md` Section 6, item 4 (tag `@Telegraphprotoc`, must be substantive, at least 3 posts before submission per acceptance criteria 7.1).

Format: date, milestone, whether an X update was posted, link if posted.

---

| Date | Milestone | X update posted | Link |
|------|-----------|------------------|------|
| Aug 21 | Repo scaffolded, spec locked, open questions logged | No | |
| Aug 21 | Telegraph team confirmed GTs are internal/never revealed, participants judged by internal consensus bounds, not a visible target. Reinforces drift-control design. | No | |
| Aug 21 | Telegraph team confirmed FRAUD_DETECTION's GT is finalized (scoreable now), AGENT_TASK's GT is pending (not scoreable yet). Open question 6 resolved. FRAUD_DETECTION is now the sole primary DWCS intent, AGENT_TASK moved to monitor-only. | No | |
| Aug 22 | Major architecture correction (decisions D9, D10). Confirmed the scoring module has no network access, no filesystem, no shared state, receives (question, ground_truth, miner_answer) as plain text, returns a single f32, wasm32-unknown-unknown target. This invalidated the original LLM-judge-ensemble design entirely. Rewrote DWCS as a deterministic four-metric ensemble (word overlap, stopword-weighted overlap, bigram Jaccard, LCS ratio) with variance-based damping, implemented for real in dwcs/rust-module/src/lib.rs with inline unit tests. Also confirmed Sentinel cannot call DWCS directly (no channel, no access to ground_truth), redesigned Sentinel's confidence signal as multi-miner agreement instead, and split the on-chain action into Layer 1 (automatic x402 payment receipt, confirmed via docs) and Layer 2 (governance-contract flag, still an open build decision). | No | |


---

## Draft notes for future X updates (not posted yet, just capturing raw material as it happens)

Nothing yet. Add a bullet here the moment something demoable exists, then turn it into an actual post before submission.
```

---

## `docs/PROJECT_SPEC.md`

```markdown
# Project Cassandra
### Disagreement-Weighted Canonical Scoring for Telegraph Protocol
**Telegraph Hackathon, Season I, 2026 (Track 2: Script Authors + Track 3: Applications)**

Status: Pre-build spec, locked for execution
Last verified against source: Aug 21, 2026

---

## 0. How to read this document

This is the execution spec, not a pitch deck. Every claim about how Telegraph works is sourced below. Everything we could **not** verify from public docs is flagged explicitly in Section 9. Do not build against assumptions in that section without confirming in the Telegraph Discord first. That is the single biggest risk to this project: building against a guessed API surface.

---

## 1. Source material (what we actually pulled, verbatim facts only)

| # | Source | What it told us |
|---|--------|------------------|
| 1 | `hackathon.telegraphprotocol.com` | Track structure, prize pool, dates, "300+ builders registered" |
| 2 | `hackathon.telegraphprotocol.com/rules` | Judging criteria weights, guardrails, non-negotiable rules, prize breakdown |
| 3 | `hackathon.telegraphprotocol.com/supported-intents` | Full 40-intent catalog, Tier A/B split, ground-truth methods per intent |
| 4 | `telegraphprotocol.com` (main site) | Protocol mechanics: Miners, Validators, Script Authors, on-chain settlement, "1,000+ standardized AI skills" index |
| 5 | `integrate.telegraphprotocol.com` | Developer console exists. "Connect API," "Submit WASM," "Consume Intelligence" as the three literal submission paths |
| 6 | `docs.telegraphprotocol.com` | JS-rendered app shell only, could not extract page-level technical content via fetch. **Needs direct browser access before build starts.** |
| 7 | Third-party writeup, *"Telegraph Wants to Be the Visa of Machine Intelligence"* (theopensourcepress.com, Jun 8 2026) | Independent confirmation of mechanism: stake-weighted median consensus, cryptographic ground-truth checks, script authors earn a share of a 20% emission pool proportional to validator reliance, and, critically, **scripts that drift or produce volatility relative to consensus get automatically discarded by the network**, while accurate ones get automatically promoted |

---

## 2. The mechanism we are exploiting (in plain terms)

**Updated Aug 22 with the confirmed technical reality, see decision D9.** Telegraph is not a marketplace where an agent picks a provider. An agent declares an **Intent** (a domain, e.g. `FRAUD_DETECTION`), a confidence threshold, and a deadline. Telegraph routes the request to a Miner; the Miner's answer is scored against a ground-truth answer by a WASM scoring module. Higher, more consistent scores mean more routed traffic, more USDC earned, stronger Miners attracted, and the network compounds.

The scoring module itself is a sandboxed, self-contained WASM binary. It receives exactly three plain-text strings, `question`, `ground_truth`, `miner_answer`, and must return a single `f32` between 0 and 1. Critically: **it has no network access, no filesystem access, and no shared state across calls.** It cannot call an external LLM API. It cannot look anything up. It is pure, deterministic computation over the three strings it's given, nothing else. Every module also has to clear a two-stage promotion process: Stage 1 structural checks (loads correctly, scores a blank answer as exactly 0, scores a correct answer above an unrelated one, doesn't crash on adversarial input), then Stage 2, where it has to beat the currently active "champion" module on a fixed benchmark of good/bad answer pairs, on margin, win-count, and self-match strength, or it never goes live.

Two structural facts make this hackathon winnable in a non-cliche way:

**Fact A: the network's own reference example is a naive word-overlap scorer.**
Telegraph's own documentation ships a simple word-overlap scoring example explicitly framed as "a legitimate starting point you can build on." That is exactly the naive, easily-gamed approach most entrants will ship as-is. It fails on the exact case Telegraph's own testing guidance names directly: "a reworded version of the correct answer that says the same thing differently... a good scorer should still recognize this as correct." Naive word overlap does not reliably do that. Whoever builds something structurally better than word-matching, while staying inside the no-network sandbox, has a real, demonstrable, and legitimately hard-to-replicate edge.

**Fact B: disagreement-as-signal still applies, just implemented differently than originally planned.**
The original plan for DWCS assumed calling multiple LLM judges at scoring time. That's not possible inside the sandbox. The mechanism has been redesigned (D9) to compute several structurally diverse, purely algorithmic similarity signals over the same `(ground_truth, miner_answer)` pair, treat disagreement across those signals as a within-module confidence check, and fold everything into one final `f32`. The core idea, disagreement across diverse evaluators is itself useful information, survives the pivot. The implementation does not.

**Fact C: promotion is judged against a real, known benchmark, not vague "network volatility."**
The earlier assumption (from a third-party writeup) that "the network discards volatile scripts" turned out to be a simplification. The real mechanism is concrete and testable: Stage 2 compares `candidate_margin`, `candidate_wins`, and `worst_self_match` against the current champion's numbers on a fixed benchmark set. This is good news, it means we can replicate this exact benchmark methodology locally before ever registering on-chain, using the same self-match and variance checks Telegraph itself uses (see Section 7 acceptance criteria).

---

## 3. Why this specific team is positioned to win this, not just enter it

This is not a borrowed idea. It is a direct port of a mechanism the team has already shipped and had recognized by the GenLayer core team:

- **Penumbra**, a library of 20 Intelligent Contract primitives built explicitly around *"disagreement as measurement instrument."* GenLayer's Intelligent Contracts already solve the exact problem Telegraph's Script Author track is now opening up as a hackathon category: how do you get trustworthy, non-gameable consensus out of non-deterministic LLM outputs? GenLayer's answer is optimistic democracy, where validators vote, and it's disagreement among validator outputs, not a single validator's confidence, that becomes the actual trust signal.
- **Counsel**, an advocate agent for on-chain deals, judged by GenLayer consensus, proven live across multiple environments. This is a working precedent for building an agent whose entire design assumes its output will be judged adversarially and must survive that.
- **The multi-repo RL agent autonomy project**, connecting off-chain RL agents to on-chain consensus reward functions, is a direct precedent for Section 5's Application Track build (an agent that acts on verified intelligence and triggers on-chain consequences).

The reframe for the judges: *we are not proposing a clever hack. We are porting a working, previously-recognized consensus mechanism from one protocol (GenLayer) to solve a named, unsolved problem on another protocol (Telegraph), and we are doing it before the protocol team has to build it themselves.*

---

## 4. Track 2 build: the script, "Disagreement-Weighted Canonical Scoring" (DWCS)

**Architecture corrected Aug 22, see decision D9.** The original design assumed calling multiple LLM judges at scoring time. That is not possible, the scoring module runs in a sandboxed WASM environment with no network access. Everything below reflects the corrected, buildable design.

### 4.1 What it does, in one sentence
Instead of scoring a Miner's answer with one naive similarity check (the word-overlap approach Telegraph's own example ships as a starting point), DWCS computes several structurally different, purely algorithmic similarity signals between the `ground_truth` and `miner_answer` strings, and treats **disagreement across those signals** as an internal confidence check that shapes the single final score, rather than trusting any one signal blindly.

### 4.2 Mechanism

**Step 1: Multi-metric construction (the "ensemble," fully offline).**
For a given `(ground_truth, miner_answer)` pair, compute at least 4 structurally diverse, deterministic similarity metrics, no network calls, no external model, everything runs inside the WASM sandbox:
- **Exact/near-exact match check.** Case- and whitespace-normalized equality, short-circuits to 1.0.
- **Normalized word overlap.** The baseline approach (matches Telegraph's own example), what fraction of `miner_answer`'s words appear in `ground_truth`, case-insensitive.
- **N-gram Jaccard similarity.** Bigram or trigram set overlap between the two strings, catches phrase-level similarity that single-word overlap misses.
- **Stopword-down-weighted overlap.** Same as word overlap, but common filler words (the, is, a, of...) count for less, so answers can't inflate their score by padding with function words.
- **Longest common subsequence ratio,** catches reordered-but-related phrasing that n-gram overlap alone might miss.

**Step 2: Canonical score computation.**
- Compute all metrics, get a vector of scores in [0,1]
- Take a weighted combination as the baseline candidate score (weights are a tuning decision, log the chosen weights and the reason in a decision log entry when set)
- Compute variance across the metric vector
- Low variance (metrics agree) means confidently trust the combined score as-is
- High variance (metrics disagree sharply) means regress the final score toward a conservative middle value rather than trusting whichever metric happened to score highest, this is the actual gaming-resistance mechanism: an answer that game one metric (e.g. keyword-stuffing to win word overlap) while failing others (e.g. n-gram/LCS structure) gets pulled down, not rewarded
- The result of this regression **is** the single `f32` returned. There is no separate confidence field, Telegraph's interface doesn't support one (confirmed, open question 2)

**Step 3: Canary defense (the actual gaming-resistance demonstration).**
Maintain a held-out, versioned set of adversarial `(question, ground_truth, miner_answer)` triples, never exposed publicly:
- Known paraphrase examples: a correct answer reworded so it shares few literal words with `ground_truth`, this specifically targets the case Telegraph's own testing guidance names as something "a good scorer should still recognize... as correct"
- Known keyword-stuffing examples: an answer padded with terms from `ground_truth` without actually answering correctly, designed to fool naive word overlap specifically
- Document, for at least one concrete pair, that the naive word-overlap baseline gets it wrong and DWCS's combined metric gets it right, or vice versa for the keyword-stuffing case. This is the demonstrable "resistance to gaming" story for the submission write-up

**Step 4: Local benchmark replication (protects against Stage 2 rejection).**
Since Stage 2 promotion is judged by beating the current champion module's `candidate_margin`, `candidate_wins`, and `worst_self_match` on Telegraph's own fixed benchmark (see Section 2 Fact C), replicate that exact methodology locally before registering:
- Self-match test: every benchmark question's correct answer scored against itself must return at least 0.75
- Variance test: scores across a benchmark set of good/bad answer pairs must show real spread, a module returning the same number for everything is rejected outright
- Margin test: for each question, the known-good answer must score meaningfully above the known-bad answer
- Use the `go-tester` CLI Telegraph provides (`telegraph-examples/wasm-scoring-module/go-tester`) to run these checks against the actual compiled `.wasm` before ever registering on-chain, every registration is a transaction, testing first is not optional in practice

### 4.3 Which intents to target first
**Updated Aug 21 following direct confirmation from the Telegraph team.** `FRAUD_DETECTION`'s ground truth is finalized and scoreable now. `AGENT_TASK`'s ground truth is still pending on the Telegraph team's side, not scoreable yet. See decision D8 in Section 8.

Priority order, and why:
1. **`FRAUD_DETECTION`** (GT finalized). Sole primary target. Directly named as a "Risk & Trust" category, and now confirmed live and scoreable.
2. **`AI_TEXT_DETECTION`, `CONTENT_VERIFICATION`, `TEXT_AUTHENTICITY_CHECK`**. All Tier B, LLM-judge, "deterministic" claimed but almost certainly single-judge in practice today; the secondary target to fill out the "3 or fewer intents" allocation now that `AGENT_TASK` is on hold. Good candidates for DWCS to show a measurable accuracy delta against whatever the incumbent scoring looks like. Registration confirmed open for all three (open question 8).
3. **`AGENT_TASK`** (GT pending). Monitor, do not build against as a primary target yet. Revisit once the Telegraph team confirms its GT has landed. If it lands before Aug 31, it can be added as a third intent; if not, do not force it in.
4. Do **not** spread across more than 3 intents for Hackathon 1. Depth beats breadth: the guardrail requires 3+ active Miners and 100+ real Track 3 requests *per intent* for prize eligibility, so concentrating demand matters more than covering many intents thinly.

---

## 5. Track 3 build: the application, closing the loop

### 5.1 Concept
An autonomous compliance/fraud-triage agent that pays for `FRAUD_DETECTION` (plus `CONTENT_VERIFICATION`/`AI_TEXT_DETECTION` if the secondary intent is added) inference via x402 on a defined document stream (candidates below), and takes a confidence-gated action when the result clears a threshold.

**Corrected Aug 22, see decision D10.** "On-chain action" here has two distinct layers, don't conflate them:
- **Layer 1, automatic and Telegraph-native:** every paid x402 request is itself an on-chain-settled payment. The response includes a `signal_hash`, independently verifiable via `GET /engine/v1/signal/{signal_hash}` and visible on `explorer.telegraphprotocol.com`. This alone satisfies "must use Telegraph miners" and gives us a real, auditable receipt for every scored proposal, no extra integration needed.
- **Layer 2, our own addition, still an open build decision:** an explicit "flag this proposal" action written to a governance contract. Telegraph does not provide this, it has to be our own contract call into whichever governance system the chosen document stream (Section 5.2) uses. This is a real scope item, not a documentation gap, decide and record the specific contract/interface once the document stream is finalized.

This satisfies, directly, two of the hackathon's own named "High-Value Areas to Explore":
- *"On-Chain & Blockchain Intelligence Pipelines"*, explicitly called their highest-value use case
- *"Signal Quality & Verification"*, explicitly named as an area to deeply understand

### 5.2 Candidate document streams (pick one, do not build for all three)
- **On-chain governance proposals**: score proposal text plus linked evidence for fabrication/manipulation before a DAO vote executes
- **DeFi protocol disclosures / audit claims**: score claims of audit coverage, TVL figures, or security claims against on-chain reality (`TVL_LOOKUP`, `ONCHAIN_TX_LOOKUP` as cross-check intents)
- **Financial disclosure text (e.g. press releases tied to token launches)**: score for AI-generated fabrication patterns

Recommendation: **on-chain governance proposals.** It's the tightest, most demoable loop (proposal text, then a paid `FRAUD_DETECTION` request scored by DWCS, then Layer 1's automatic on-chain receipt, then optionally Layer 2's governance-contract flag if built in time), and it's the closest match to the team's existing Counsel work (an agent whose entire purpose is being judged on an on-chain deal).

### 5.3 Why building both tracks together is the actual strategy, not scope creep
- The Application Track's judging criteria require **"must use Telegraph miners"** and reward "creativity and usefulness" plus real usage.
- The Miner/Script guardrail requires **3+ active Miners and 100+ real requests from Track 3 applications** per intent to be eligible for global cash prizes at all.
- By owning both the scoring script (Track 2) and a live consumer of it (Track 3), we are not hoping strangers generate our qualifying demand. We generate it ourselves, deterministically, before the Sep 7 deadline, and we can demonstrate the full flywheel (Miner, Script, Application, real usage, re-ranking) as a single coherent narrative in the submission. That narrative, proving the flywheel works end-to-end, is literally what the rules page states is "the purpose of this hackathon."

---


## 6. Non-negotiables

These are hard constraints. If a build decision conflicts with one of these, the decision is wrong, not the constraint.

1. **No mocked or simulated Miner data in the Track 3 application, ever.** The rules page states this explicitly as rule #1 and it is a disqualifying condition, not a style preference.
2. **The scoring module has no network access, no filesystem access, and no shared state across calls, ever.** Confirmed directly in Telegraph's docs. Do not write code that assumes it can call an LLM, fetch a URL, or read a file from inside `rank_answer`. Everything the module needs must be computable from the three input strings alone, or bundled as static data inside the compiled binary.
3. **Output is a single `f32` between 0 and 1, nothing else.** No struct, no pointer, no confidence field. Any internal confidence/disagreement signal must be folded into that one number before returning, not published separately.
4. **Target `wasm32-unknown-unknown`, never `wasm32-wasip1`.** A WASI build has OS-function imports and will fail to instantiate on Telegraph's node. Verify with `wasm-tools print <file>.wasm | grep -c '(import'`, must print `0` before registering.
5. **32MB binary size cap.** Keep the compiled module well under this; a good scoring module should be nowhere near the limit.
6. **Test locally before ever registering on-chain.** Every registration is a transaction. Use the `go-tester` CLI (`telegraph-examples/wasm-scoring-module/go-tester`) or an equivalent local harness to check empty-answer, wrong-answer, self-match, and paraphrase cases before submitting anything to the chain.
7. **Miners and the script must remain live and operational through all of Track 3.** No demo-day-only uptime. If we can't commit to running infrastructure continuously from Aug 31 through Sep 7, we should not enter this build.
8. **All progress updates must be posted on X, tagged `@Telegraphprotoc`, and must be genuinely substantive.** This is 25% of the Script Author score and part of the Miner Track score. Treat this as a build requirement with its own schedule, not an afterthought at submission time.
9. **No artificial inflation of metrics.** This includes not gaming our own Application Track's "users acquired & activity" numbers with synthetic traffic, and not padding the 100-request guardrail with non-real requests. If we can't hit the guardrail organically, we say so rather than fake it.
10. **Concentrate on 3 or fewer intents.** Spreading across many intents thins out the requests-per-intent guardrail and dilutes the demo narrative. Depth over breadth is a non-negotiable, not a preference, because the guardrail math punishes breadth directly.
11. **Do not build against assumed API/SDK details.** Anything still open in Section 9 must be confirmed against the actual docs site or the Discord before code is written against it. Getting an interface wrong costs days we don't have. (Note: the WASM interface and on-chain settlement mechanism are now confirmed, see Section 9, this rule remains in force for anything still unresolved.)
12. **Join and stay active in the official Discord.** Not optional, and it's also our fastest path to resolving anything still open.

---

## 7. Acceptance criteria

### 7.1 Script Author track (DWCS), must all be true to consider this "submission ready"
- [ ] DWCS is implemented as an actual `.wasm` binary exporting `alloc`, `dealloc`, and `rank_answer` per the confirmed interface, compiled for `wasm32-unknown-unknown`, verified to have zero imports
- [ ] DWCS passes all four Stage 1 structural checks locally before registration: loads and exports the required functions, blank/empty answer scores exactly 0, a correct answer scores strictly above an unrelated one, handles long/emoji/non-English input without crashing
- [ ] DWCS passes a locally-replicated Stage 2 check against a self-built benchmark: self-match on a perfect answer scores at least 0.75, scores show real variance across the benchmark (not a constant output), and the module separates good from bad answers by a clear, consistent margin
- [ ] A documented canary test set exists (versioned, held out, not visible to Miners) demonstrating at least one concrete case where naive word overlap would have failed (either scoring a valid paraphrase too low, or scoring a keyword-stuffed wrong answer too high) and DWCS's combined metric did not
- [ ] At least 4 structurally diverse similarity metrics are computed and combined per scored pair, not just repeated variants of the same metric
- [ ] DWCS is registered and live scoring `FRAUD_DETECTION` before Track 1 & 2 close on Aug 31
- [ ] At least 3 substantive, tagged X progress updates posted before submission, each showing a distinct build milestone (not 3 posts the same day)

### 7.2 Application track, must all be true
- [ ] Application makes real, paid x402 requests to real, live Telegraph Miners for its chosen intents, verified against actual API responses and `signal_hash` receipts, logged as evidence
- [ ] End-to-end flow demoable: document input, then x402-paid Telegraph Intent request, then Miner response, then DWCS score, then Layer 1 automatic on-chain receipt (`signal_hash`, verifiable via `GET /engine/v1/signal/{signal_hash}`), then, if Layer 2 is built, a visible governance-contract flag transaction
- [ ] At least 100 real requests generated against the target intent(s) before Sep 7, attributable to this application (supports the Track 1/2 guardrail as well)
- [ ] Confidence threshold and deadline parameters are configurable and documented, with a written rationale for the chosen default threshold
- [ ] A clear, non-gamed usage/activity metric exists for judging ("users acquired & activity," "usage and adoption")

### 7.3 Cross-cutting
- [ ] Both submissions explicitly reference each other in their write-ups (the script's submission cites the application as its real-demand proof; the application's submission cites the script as its scoring layer). This is the flywheel narrative and it should not be left implicit
- [ ] Nothing in either submission uses simulated Miner data at any point, including in early testing artifacts that might get surfaced in the demo

---

## 8. Decision log

| # | Decision | Rationale | Alternatives rejected | Date |
|---|----------|-----------|------------------------|------|
| D1 | Skip Miner Track entirely | Wrapping an API is the default, most crowded, least defensible play. The team's real edge is in consensus/scoring mechanisms, not API integration. | Building a Miner as a "safe" fallback entry, rejected because it dilutes focus across 3 tracks in a 10-day window and doesn't leverage unique expertise | Aug 21 |
| D2 | Target `FRAUD_DETECTION` and `AGENT_TASK` first for DWCS | Both explicitly marked as having no ground-truth methodology yet: first-mover advantage on defining protocol infrastructure, not just competing within an existing rubric | Targeting a well-established Tier A intent (e.g. `STOCK_PRICE`), rejected, those are deterministic WASM exact-match, no room for a judge-quality argument at all | Aug 21 |
| D8 | Drop `AGENT_TASK` as a primary target, make `FRAUD_DETECTION` the sole primary intent, add a Tier 3 intent as secondary | Telegraph team confirmed directly: `FRAUD_DETECTION`'s GT is finalized and scoreable now; `AGENT_TASK`'s GT is still pending on their side. Building against an intent whose GT isn't finished yet risks wasted work and a script that can't be evaluated. Supersedes D2's original two-intent framing. | Keeping `AGENT_TASK` as a co-primary target and hoping the GT lands in time, rejected, too much schedule risk with Track 1/2 closing Aug 31 | Aug 21 |
| D9 | Pivot DWCS from a multi-LLM-judge ensemble to a multi-metric deterministic algorithmic ensemble (exact match, word overlap, n-gram Jaccard, stopword-weighted overlap, LCS ratio) | Telegraph's own docs confirm the scoring module runs in a sandboxed WASM environment with no network access, no filesystem access, no shared state. Calling an external LLM judge from inside `rank_answer` is not possible. The core "disagreement as signal" idea is preserved, computed across diverse deterministic metrics instead of diverse LLM judge passes. This is not a downgrade of the idea, it's the only version of it that can actually run where it needs to run. | Keeping the LLM-judge ensemble design and hoping for some workaround (e.g. precomputing judge calls outside the module and passing results in), rejected, the module only ever receives `question`/`ground_truth`/`miner_answer` as plain text, there is no channel to pass in precomputed judge scores | Aug 22 |
| D10 | Split Sentinel's "on-chain action" into two explicit layers: automatic x402 payment settlement (Layer 1) and an optional separate governance-contract flag write (Layer 2) | Telegraph's on-chain settlement is the x402 payment receipt (`signal_hash`), confirmed via docs and team reply. It does not include any DAO-specific action. Conflating the two in the original spec risked building toward a Telegraph API that doesn't exist. Making this two layers means Layer 1 alone already satisfies "must use Telegraph miners" with a real on-chain receipt, and Layer 2 is scoped as its own decision once a document stream is finalized, not an assumed given | Aug 22 |
| D11 | Adopt strict repo hygiene: no em-dashes anywhere, professional conventional commit messages with no AI-tool attribution, and gitignore every `.md` file except files literally named `README.md` | This is a hackathon submission judges and other builders will actually read and clone. A clean, professional-looking repo history and file structure is part of the credibility argument alongside the technical one. Keeping internal planning docs (spec, decision log, task list, agent instructions) untracked and local avoids cluttering the public repo with process artifacts while still letting them exist and inform the build | Aug 22 |
| D3 | Disagreement is a signal about Miner output, never license for our script's own volatility | Source #7 confirms Telegraph's meta-layer discards drifting/volatile scripts automatically. Conflating "detecting Miner gaming" with "our script being noisy" would get us discarded by the very mechanism we're trying to exploit. | Letting ensemble variance directly set our published canonical score without a stability check, rejected, too risky given confirmed discard mechanism | Aug 21 |
| D4 | Build Track 2 (script) and Track 3 (application) together, single team, single narrative | Guardrail requires 3+ Miners and 100+ real requests per intent; owning both ends lets us generate qualifying demand deterministically rather than hoping for organic traffic within a 7-day Track 3 window | Submitting only the script and hoping other hackathon apps generate demand for our intents, rejected, too much dependency risk with only ~10 days of runway before Track 1/2 closes | Aug 21 |
| D5 | Pick on-chain governance proposals as the Application Track document stream | Tightest demoable loop, closest match to prior shipped work (Counsel, an advocate agent judged by consensus on-chain), easiest to source real documents (public governance forums) without needing private data partnerships | DeFi disclosure claims and financial press releases, both viable but require either audit-data partnerships or harder-to-verify ground truth within our timeline; deprioritized, not discarded | Aug 21 |
| D6 | Concentrate on 3 or fewer intents total across both tracks | Guardrail math (3+ Miners, 100+ requests *per intent*) directly punishes spreading thin; a strong showing on 2 to 3 intents beats a weak showing on 6 | Covering more intents for "coverage" optics, rejected | Aug 21 |
| D7 | Do not finalize WASM script interface details in this document | `docs.telegraphprotocol.com` is a JS-rendered SPA that did not yield extractable technical content via automated fetch. Writing code against a guessed interface risks wasted build days. | Guessing at a plausible schema based on similar protocols (e.g. Bittensor-style subnet scripts) and building against it, rejected, explicitly a "cannot afford mistakes" risk per the brief | Aug 21 |

---

## 9. Open questions, must resolve before writing code

These are not filled in with best guesses anywhere above. Resolve via the actual rendered docs site (open in a real browser, not fetched as static HTML) and/or the Telegraph Discord, in this order of urgency:

1. ~~Exact WASM script interface/contract.~~ **RESOLVED, Aug 22.** The module receives `question`, `ground_truth`, `miner_answer` as three plain-text strings via `(pointer, length)` pairs. It exports `alloc`, `dealloc`, `rank_answer`, and returns a single `f32` between 0 and 1. Target `wasm32-unknown-unknown`, 32MB cap, no network/filesystem/shared-state access. Source: `docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module`. See decision D9, this required a full redesign of DWCS's mechanism.
2. ~~Whether Telegraph natively supports a "confidence" or "contested" field in script output.~~ **RESOLVED, Aug 22.** No. Output is a single `f32`, nothing else.
3. ~~Whether re-evaluation/re-sampling of a "contested" output is possible.~~ **RESOLVED, Aug 22.** No, confirmed directly by the team. One invocation per epoch, every score is final.
4. ~~The exact on-chain settlement/receipt mechanism.~~ **RESOLVED, Aug 22.** x402 payment flow: request, 402 challenge, signed USDC payment, retry with `PAYMENT-SIGNATURE`, response includes a `signal_hash` verifiable via `GET /engine/v1/signal/{signal_hash}` and visible on `explorer.telegraphprotocol.com`. This is Layer 1 of Sentinel's on-chain action, see decision D10. Layer 2 (a governance-specific flag action) remains a separate, undecided build item, not a documentation gap.
5. **How "Number of applications built on your Miner" and "Total requests served" are actually measured/attributed** for Miner Track judging (not directly our track, but affects how we should instrument the Application Track build to correctly attribute its Telegraph calls if we ever add a thin Miner later).
6. ~~Whether Script Authors can submit scores for intents where no ground truth exists yet.~~ **RESOLVED, Aug 21.** Telegraph team confirmed GTs are held internally and never revealed to participants. `FRAUD_DETECTION`'s GT is finalized and scoreable now. `AGENT_TASK`'s GT is still pending on their side. See decision D8 in Section 8 and the updated priority order in Section 4.3.
7. **Registration status.** Confirm whether the team is already registered (site claims 300+ registered builders and registration unlocks early track access, task specs, and a private Discord channel).
8. ~~GT status of Tier 3 secondary intents.~~ **PARTIALLY RESOLVED, Aug 22, not a build blocker.** Telegraph team confirmed registration is open for `AI_TEXT_DETECTION`, `CONTENT_VERIFICATION`, and `TEXT_AUTHENTICITY_CHECK` ("Yes you can register for those"). Registration is a deploy-time administrative step, it does not block local work on `dwcs/src/`, which is intent-agnostic until the WASM interface layer. Individual GT-finalized status per intent is still unconfirmed, same open question as item 6 was for `FRAUD_DETECTION`/`AGENT_TASK`. Team also asked which track we're building on (Miner or WASM/Script Author); replied confirming WASM/Script Author plus a separate Application track submission.
9. **Real wasm32-unknown-unknown build, NOT YET VERIFIED.** `cargo test` (host) passes all 8 unit tests in `dwcs/rust-module/src/lib.rs`, that was actually run. The real `cargo build --release --target wasm32-unknown-unknown` was attempted in the environment this scaffold was built in and failed with "can't find crate for core" because `rustup` wasn't available to install the target, not because of a reported logic error, but that is not the same as a confirmed working WASM build. **Do this before registering anything on-chain:** `rustup target add wasm32-unknown-unknown`, then the release build, then the `wasm-tools` zero-import check. Commands are all in `dwcs/rust-module/README.md`.
10. **`@x402/fetch` / `@x402/evm` version pins are unverified placeholders** in `package.json`. Package names and import paths are confirmed from Telegraph's docs, the specific version numbers were never checked against the real npm registry. Run `npm view @x402/fetch versions` (and the same for `@x402/evm`) before `npm install`.

---

## 10. Execution timeline (compressed against actual hackathon dates)

| Date | Milestone |
|------|-----------|
| Now to Aug 22 | Resolve Section 9, items 1-3 and 6 especially. Register / confirm registration. Join Discord. |
| Aug 22 to Aug 25 | DWCS ensemble logic built and tested locally against a hand-built canary set (before touching the live network) |
| Aug 25 to Aug 28 | DWCS deployed as a live Telegraph script against chosen intent(s); first X progress update posted |
| Aug 28 to Aug 31 | Stability/drift testing against network consensus; second X progress update posted; Track 1 & 2 close Aug 31: DWCS must be live and scoring by this date |
| Aug 31 to Sep 2 | Application Track opens. Build the governance-proposal ingestion plus confidence-gated on-chain action flow |
| Sep 2 to Sep 5 | End-to-end testing, generate real request volume against target intent(s) to clear the 100-request guardrail; third X progress update |
| Sep 5 to Sep 7 | Buffer for bugs, final demo recording, submission writeups for both tracks (cross-referencing each other per Section 7.3) |
| Sep 7, 12:00 UTC | Submissions close |
| Sep 8 to Sep 18 | Winner selection window (no action needed, but Discord activity should continue) |
| Sep 19 to Sep 25 | Winners announced, prizes distributed |

---

## 11. What "winning the argument" looks like in the submission write-up

Not a build note, a framing note. The write-up for both submissions should make three claims explicit, in this order:

1. We identified a named, unsolved problem the protocol itself admits exists (`GT: TBD`, `No GT`) rather than competing inside an already-solved rubric.
2. Our solution is a working port of a previously-shipped, externally-recognized consensus mechanism (GenLayer/Penumbra's disagreement-as-signal, Counsel's on-chain judged advocate agent), not a novel, unproven idea invented for this hackathon. That's a credibility argument judges can verify.
3. We proved the flywheel the hackathon exists to test: Miner-equivalent signal, ranked by our script, consumed by our own live application, generating real demand, as a single closed loop, which is the literal stated purpose of Hackathon 1 per the rules page.
```

---

## `docs/TASKS.md`

```markdown
# Tasks

Mirrors `PROJECT_SPEC.md` Section 10 and the corrected architecture from decisions D9/D10. Check items off as they land for real, and add a corresponding row to `PROGRESS_LOG.md` when you do.

## Now to Aug 22
- [x] Resolve OPEN_QUESTIONS.md items 1, 2, 3, and 4, all RESOLVED Aug 22 with confirmed docs/team answers. See decisions D9 (DWCS pivoted to a deterministic multi-metric ensemble, LLM judges are impossible in the no-network WASM sandbox) and D10 (Sentinel's on-chain action split into Layer 1/Layer 2).
- [x] Resolve OPEN_QUESTIONS.md item 6, RESOLVED Aug 21: FRAUD_DETECTION GT finalized (scoreable now), AGENT_TASK GT pending (not scoreable yet). FRAUD_DETECTION is now the sole primary intent.
- [ ] Pick the secondary intent from the Tier 3 list (AI_TEXT_DETECTION / CONTENT_VERIFICATION / TEXT_AUTHENTICITY_CHECK), registration confirmed open for all three (item 8), per updated PROJECT_SPEC.md Section 4.3
- [ ] Confirm registration status (item 7)
- [ ] Join and stay active in the official Telegraph Discord
- [ ] Verify real `@x402/fetch` / `@x402/evm` npm versions before `npm install`, see `app/README.md`

## Aug 22 to Aug 25
- [x] `dwcs/rust-module/src/lib.rs` implemented: bump allocator, `alloc`/`dealloc`/`rank_answer`, four-metric deterministic ensemble (word overlap, stopword-weighted overlap, bigram Jaccard, LCS ratio), variance-based damping, inline unit tests, runnable now with `cargo test`
- [x] `dwcs/src/prototype.ts` TS mirror implemented and unit tested, for fast metric tuning
- [ ] Real canary dataset populated in `dwcs/canaries/dataset.jsonl` (at least the minimum viable set described in `dwcs/canaries/README.md`, matching the real `question`/`ground_truth`/`miner_answer` format)
- [ ] Build a small internal benchmark set (multiple questions, each with a known-good and known-bad answer) and check self-match ≥ 0.75 and real score variance, replicating Telegraph's own Stage 2 promotion criteria locally

## Aug 25 to Aug 28
- [ ] Build the real `.wasm` binary: `cargo build --release --target wasm32-unknown-unknown`, verify zero imports with `wasm-tools`
- [ ] Test against Telegraph's `go-tester` harness (`telegraph-examples/wasm-scoring-module/go-tester`) with the full recommended test case set (exact match, wrong answer, empty answer, reworded answer, quality-ranked pairs)
- [ ] Register DWCS for `FRAUD_DETECTION` via `integrate.telegraphprotocol.com` or `registerWasm(...)` directly
- [ ] First X progress update posted (tag `@Telegraphprotoc`), logged in `PROGRESS_LOG.md`

## Aug 28 to Aug 31
- [ ] Confirm registration status moved from `pending` to `active` (check via explorer/API with the `registrationId`), if `rejected`, read the recorded reason and fix before re-registering
- [ ] Second X progress update posted
- [ ] **Track 1 & 2 close Aug 31.** DWCS must be `active` (live champion) by this date.

## Aug 31 to Sep 2
- [ ] Application Track opens
- [ ] Decide the specific document stream/governance target (`PROJECT_SPEC.md` Section 5.2), this decision also unblocks Layer 2 in `app/src/onchain/action.ts`
- [ ] `app/src/ingest/governance_source.ts` implemented against the chosen real source
- [ ] `app/src/scoring/telegraph_client.ts` tested against the live devnode with a real funded testnet wallet
- [ ] Layer 2 `executeLayer2GovernanceFlag` implemented against the real governance contract chosen above

## Sep 2 to Sep 5
- [ ] End-to-end flow tested live: proposal in, x402-paid multi-miner FRAUD_DETECTION request, agreement-based triage decision, Layer 1 receipt collected, Layer 2 flag executed if escalated
- [ ] Real request volume generated against target intent(s), tracked toward the 100-request guardrail
- [ ] Third X progress update posted

## Sep 5 to Sep 7
- [ ] Bug fixing buffer
- [ ] Final demo recording
- [ ] Submission write-ups for both tracks, each explicitly referencing the other (per acceptance criteria 7.3)
- [ ] Walk through `PROJECT_SPEC.md` Section 7 (7.1, 7.2, 7.3) checklist in full before submitting

## Sep 7, 12:00 UTC
- [ ] Submissions in
```

---

## `dwcs/README.md`

```markdown
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
```

---

## `dwcs/canaries/README.md`

```markdown
# Canary dataset

Held-out, versioned adversarial examples used to demonstrate DWCS's gaming-resistance, each is a `(question, ground_truth, miner_answer)` triple matching Telegraph's real scoring module interface (see `dwcs/rust-module/src/lib.rs`).

## Rules

- **Never expose this dataset to Miners or the public network.** If a Miner can see the canaries, it can optimize against them and the whole defense collapses.
- **Version everything.** When you add or change a canary, bump the version field and keep the old one if it's still a valid test case. Don't silently mutate an existing canary's `id`.
- Each canary needs a clear `expectedOutcome`. If you can't confidently state what the correct score direction is, it's not a usable canary yet, it's just an ambiguous example.
- Mix `paraphrase` and `keyword_stuffing` types. A canary set that's all one type only tests one failure mode.

## Format

See `dataset.example.jsonl` for the shape. Real data goes in `dataset.jsonl` (gitignored, see root `.gitignore`), never commit real canaries to a public repo.

## Minimum viable set for acceptance criteria 7.1

At least one concrete, documented case demonstrating naive word overlap would have failed (either scoring a valid paraphrase too low, or scoring a keyword-stuffed wrong answer too high) and DWCS's combined metric did not. Both `dwcs/rust-module/src/lib.rs` and `dwcs/src/prototype.ts` already include unit tests covering this pattern directly, use the canary set to extend that coverage with real, reviewed examples rather than synthetic test-only strings.

```

---

## `dwcs/canaries/dataset.example.jsonl`

```text
{"id": "example_001", "type": "paraphrase", "question": "PLACEHOLDER question text", "ground_truth": "PLACEHOLDER correct answer", "miner_answer": "PLACEHOLDER: a correct answer reworded so it shares few literal words with ground_truth", "expectedOutcome": "should_score_high", "notes": "PLACEHOLDER. Naive word overlap would likely score this low since few words match literally, DWCS's combined metric (especially LCS ratio) should still recognize it as correct. Replace with a real, reviewed example."}
{"id": "example_002", "type": "keyword_stuffing", "question": "PLACEHOLDER question text", "ground_truth": "PLACEHOLDER correct answer", "miner_answer": "PLACEHOLDER: an answer padded with keywords from ground_truth without actually answering correctly", "expectedOutcome": "should_score_low", "notes": "PLACEHOLDER. Naive word overlap would likely score this artificially high since many keywords match, DWCS's variance-based damping across metrics should catch that bigram/LCS structure doesn't support it and pull the score down. Replace with a real, reviewed example."}
```

---

## `dwcs/rust-module/Cargo.lock`

```text
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "dwcs-scoring-module"
version = "0.1.0"
```

---

## `dwcs/rust-module/Cargo.toml`

```toml
[package]
name = "dwcs-scoring-module"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[profile.release]
opt-level = "z"
lto = true
panic = "abort"
strip = true

# No dependencies on purpose. The compiled module must not depend on
# anything outside itself (no dynamic imports of OS functions, no threads),
# per docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module.
```

---

## `dwcs/rust-module/README.md`

```markdown
# dwcs-scoring-module

The real, deployable DWCS implementation. This is what gets compiled to `.wasm` and registered on Telegraph.

## Test on the host (no WASM tooling needed)

```
cargo test
```

The `#![cfg_attr(target_arch = "wasm32", no_std)]` attribute at the top of `src/lib.rs` means host test builds get full `std`, only the actual wasm32 build is `no_std`. This runs every unit test in `src/lib.rs`'s `scoring::tests` module directly.

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
```

---

## `dwcs/rust-module/src/lib.rs`

```rust
//! DWCS: Disagreement-Weighted Canonical Scoring.
//!
//! Real, deployable Telegraph scoring module. See docs/PROJECT_SPEC.md
//! Section 4 for the full design rationale, and decision D9 for why this
//! is a deterministic multi-metric ensemble rather than an LLM-judge
//! ensemble: the module runs with no network access, no filesystem access,
//! and no shared state across calls, confirmed at
//! docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module.
//!
//! `#![no_std]` only applies to the actual wasm32 build. Running
//! `cargo test` on the host uses std normally, so the scoring logic itself
//! (in the `scoring` module below) is fully unit-testable without any WASM
//! tooling. This is deliberate: it satisfies the "test locally before
//! registering on-chain" non-negotiable with a plain `cargo test`, in
//! addition to the `go-tester` harness Telegraph provides.
//!
//! VERIFICATION STATUS (honest, not glossed over): `cargo test` (host,
//! x86_64) passes all 8 tests as of this writing, that's real and was
//! actually run, not assumed. The real `wasm32-unknown-unknown` build
//! could NOT be verified in the environment this was written in, no
//! `rustup` was available there to install the target, and the target's
//! `core` crate wasn't present. Attempting `cargo build --release --target
//! wasm32-unknown-unknown` there failed with "can't find crate for core",
//! which is the expected failure mode for a missing target, not a
//! reported logic error, but it means the actual WASM compile has not
//! been confirmed to succeed. Run that build yourself, plus `wasm-tools`'s
//! zero-import check, before registering anything on-chain. Don't skip
//! this on the assumption that host tests passing is equivalent.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
use core::panic::PanicInfo;

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

// ---------------------------------------------------------------------
// Memory: a bump allocator, wasm32 only.
// WASM functions can only pass numbers, not strings, so the node needs
// somewhere in this module's own memory to write the question/ground
// truth/answer text before calling rank_answer. This mirrors Telegraph's
// own reference example exactly.
// ---------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const HEAP_SIZE: usize = 1 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
static mut HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
#[cfg(target_arch = "wasm32")]
static mut HEAP_OFFSET: usize = 0;

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn alloc(size: i32) -> i32 {
    let size = size.max(0) as usize;
    unsafe {
        let aligned = (HEAP_OFFSET + 3) & !3;
        if aligned + size > HEAP_SIZE {
            HEAP_OFFSET = 0;
        } else {
            HEAP_OFFSET = aligned;
        }
        let ptr = core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(HEAP_OFFSET);
        HEAP_OFFSET += size;
        ptr as i32
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn dealloc(_ptr: i32, _size: i32) {}

#[cfg(target_arch = "wasm32")]
unsafe fn read_str<'a>(ptr: i32, len: i32) -> &'a str {
    unsafe {
        let slice = core::slice::from_raw_parts(ptr as *const u8, len.max(0) as usize);
        core::str::from_utf8_unchecked(slice)
    }
}

// ---------------------------------------------------------------------
// Scoring logic. Pure `core`-only functions, no allocation beyond fixed
// stack arrays, no I/O, no network, no shared state. Fully testable on
// the host target with `cargo test` (std is available there since the
// `no_std` attribute above only applies to the wasm32 build).
// ---------------------------------------------------------------------

pub mod scoring {
    // Explicit import for absolute clarity in a no_std context. The core
    // prelude normally brings this in automatically, this just removes any
    // doubt given this module couldn't be fully verified against the real
    // wasm32-unknown-unknown target in the environment this was written in
    // (see the crate-level note at the top of this file).
    use core::iter::Iterator;

    /// Cap on how many words we consider per input. Keeps every metric
    /// below bounded time/stack cost even for the "tens of KB" adversarial
    /// inputs Telegraph explicitly tests scoring modules against. Answers
    /// longer than this are not rejected, just truncated for scoring
    /// purposes, this is a deliberate safety cap, not a correctness bug.
    pub const MAX_WORDS: usize = 100;

    /// Below this variance across the four metrics, treat them as agreeing
    /// and trust the plain combined score. Above it, damp toward a more
    /// conservative blend, this is the actual gaming-resistance mechanism:
    /// an answer that games one metric while failing the others gets
    /// pulled down rather than rewarded. Starting value, tune against the
    /// canary set once populated and record the tuning as a decision log
    /// entry, not a silent edit.
    pub const VARIANCE_DAMPING_THRESHOLD: f32 = 0.05;

    const STOPWORDS: [&str; 24] = [
        "the", "a", "an", "is", "are", "was", "were", "of", "in", "on", "at", "to", "and", "or",
        "but", "this", "that", "it", "i", "you", "he", "she", "we", "they",
    ];

    fn is_stopword(w: &str) -> bool {
        STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(w))
    }

    /// Splits on whitespace into at most MAX_WORDS slices, returns the count.
    fn tokenize<'a>(s: &'a str, buf: &mut [&'a str; MAX_WORDS]) -> usize {
        let mut n = 0;
        for w in s.split_whitespace() {
            if n >= MAX_WORDS {
                break;
            }
            buf[n] = w;
            n += 1;
        }
        n
    }

    fn eq_ci(a: &str, b: &str) -> bool {
        a.eq_ignore_ascii_case(b)
    }

    /// Fraction of answer words that also appear anywhere in ground truth.
    /// This is Telegraph's own reference-example metric, included as one
    /// input among several rather than the whole story.
    fn word_overlap(answer: &[&str], truth: &[&str]) -> f32 {
        if answer.is_empty() {
            return 0.0;
        }
        let mut matched = 0u32;
        for w in answer {
            if truth.iter().any(|t| eq_ci(t, w)) {
                matched += 1;
            }
        }
        matched as f32 / answer.len() as f32
    }

    /// Same idea as word_overlap, but stopwords count for less, so an
    /// answer can't inflate its score by padding with function words.
    fn stopword_weighted_overlap(answer: &[&str], truth: &[&str]) -> f32 {
        if answer.is_empty() {
            return 0.0;
        }
        let mut matched_weight = 0.0f32;
        let mut total_weight = 0.0f32;
        for w in answer {
            let weight = if is_stopword(w) { 0.3 } else { 1.0 };
            total_weight += weight;
            if truth.iter().any(|t| eq_ci(t, w)) {
                matched_weight += weight;
            }
        }
        if total_weight == 0.0 {
            0.0
        } else {
            matched_weight / total_weight
        }
    }

    /// Approximate Jaccard similarity over consecutive-word bigrams.
    /// Catches phrase-level similarity single-word overlap misses, and
    /// catches keyword-stuffing that scrambles word order to inflate
    /// word_overlap without preserving actual phrasing.
    fn bigram_jaccard(answer: &[&str], truth: &[&str]) -> f32 {
        let a_bigrams = answer.len().saturating_sub(1);
        let t_bigrams = truth.len().saturating_sub(1);
        if a_bigrams == 0 || t_bigrams == 0 {
            return 0.0;
        }
        let mut matched = 0u32;
        let mut used = [false; MAX_WORDS];
        for i in 0..a_bigrams {
            for j in 0..t_bigrams {
                if used[j] {
                    continue;
                }
                if eq_ci(answer[i], truth[j]) && eq_ci(answer[i + 1], truth[j + 1]) {
                    used[j] = true;
                    matched += 1;
                    break;
                }
            }
        }
        let union = a_bigrams as u32 + t_bigrams as u32 - matched;
        if union == 0 {
            0.0
        } else {
            matched as f32 / union as f32
        }
    }

    /// Longest common subsequence over words (not characters), normalized
    /// by the longer of the two word counts. Catches reordered-but-related
    /// phrasing (a valid paraphrase) that n-gram overlap alone can miss,
    /// this is the metric most directly aimed at the case Telegraph's own
    /// docs name: "a reworded version of the correct answer that says the
    /// same thing differently... a good scorer should still recognize this
    /// as correct."
    fn lcs_ratio(answer: &[&str], truth: &[&str]) -> f32 {
        let n = answer.len();
        let m = truth.len();
        if n == 0 || m == 0 {
            return 0.0;
        }
        // (MAX_WORDS+1) x (MAX_WORDS+1) table, bounded, lives on the stack.
        let mut dp = [[0u16; MAX_WORDS + 1]; MAX_WORDS + 1];
        for i in 1..=n {
            for j in 1..=m {
                if eq_ci(answer[i - 1], truth[j - 1]) {
                    dp[i][j] = dp[i - 1][j - 1] + 1;
                } else {
                    dp[i][j] = if dp[i - 1][j] > dp[i][j - 1] {
                        dp[i - 1][j]
                    } else {
                        dp[i][j - 1]
                    };
                }
            }
        }
        let lcs_len = dp[n][m] as f32;
        let denom = if n > m { n } else { m } as f32;
        lcs_len / denom
    }

    fn mean(values: &[f32]) -> f32 {
        values.iter().sum::<f32>() / values.len() as f32
    }

    fn variance(values: &[f32], avg: f32) -> f32 {
        mean(&to_fixed4(values.iter().map(|v| (v - avg) * (v - avg))))
    }

    // core has no Vec, so a tiny fixed-size collector for the 4-element
    // metric arrays used above. Avoids pulling in `alloc`.
    fn to_fixed4<I: Iterator<Item = f32>>(iter: I) -> [f32; 4] {
        let mut out = [0.0f32; 4];
        for (i, v) in iter.enumerate().take(4) {
            out[i] = v;
        }
        out
    }

    /// Combines the four metrics into a single final score, damping toward
    /// a conservative blend when the metrics disagree sharply. This is
    /// where "disagreement as measurement instrument" actually lives:
    /// low variance across metrics means trust the mean, high variance
    /// means an answer likely games one specific metric while failing the
    /// others, so pull the score down rather than reward whichever metric
    /// scored it highest.
    fn combine(metrics: [f32; 4]) -> f32 {
        let avg = mean(&metrics);
        let var = variance(&metrics, avg);

        let min = metrics.iter().cloned().fold(1.0f32, f32::min);

        if var <= VARIANCE_DAMPING_THRESHOLD {
            avg.clamp(0.0, 1.0)
        } else {
            // Damped blend: half the mean, half the minimum. An answer
            // that scores well on some metrics and poorly on others does
            // not get to keep the optimistic mean.
            (0.5 * avg + 0.5 * min).clamp(0.0, 1.0)
        }
    }

    /// Top-level entry point for the pure scoring logic. Returns a value
    /// in [0, 1]. An empty or blank answer always scores exactly 0, per
    /// Telegraph's required behavior.
    pub fn score_pair(ground_truth: &str, miner_answer: &str) -> f32 {
        if miner_answer.trim().is_empty() {
            return 0.0;
        }
        if ground_truth.trim().eq_ignore_ascii_case(miner_answer.trim()) {
            return 1.0;
        }

        let mut a_buf: [&str; MAX_WORDS] = [""; MAX_WORDS];
        let mut t_buf: [&str; MAX_WORDS] = [""; MAX_WORDS];
        let a_n = tokenize(miner_answer, &mut a_buf);
        let t_n = tokenize(ground_truth, &mut t_buf);
        let answer = &a_buf[..a_n];
        let truth = &t_buf[..t_n];

        let metrics = [
            word_overlap(answer, truth),
            stopword_weighted_overlap(answer, truth),
            bigram_jaccard(answer, truth),
            lcs_ratio(answer, truth),
        ];

        combine(metrics)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn empty_answer_scores_zero() {
            assert_eq!(score_pair("Paris is the capital of France.", ""), 0.0);
            assert_eq!(score_pair("Paris is the capital of France.", "   "), 0.0);
        }

        #[test]
        fn exact_match_scores_one() {
            assert_eq!(
                score_pair("Paris is the capital of France.", "Paris is the capital of France."),
                1.0
            );
        }

        #[test]
        fn correct_answer_beats_unrelated_answer() {
            let gt = "Paris is the capital of France.";
            let good = score_pair(gt, "The capital of France is Paris.");
            let bad = score_pair(gt, "Bananas are yellow and grow on trees.");
            assert!(good > bad, "expected good ({good}) > bad ({bad})");
        }

        #[test]
        fn reworded_correct_answer_still_scores_reasonably_high() {
            // This is the exact case Telegraph's own docs call out: "a
            // reworded version of the correct answer that says the same
            // thing differently... a good scorer should still recognize
            // this as correct."
            let gt = "The mitochondria is the powerhouse of the cell.";
            let reworded = "Mitochondria act as the cell's powerhouse.";
            let unrelated = "Stock prices fell sharply on Tuesday.";
            let reworded_score = score_pair(gt, reworded);
            let unrelated_score = score_pair(gt, unrelated);
            assert!(
                reworded_score > unrelated_score,
                "expected reworded ({reworded_score}) > unrelated ({unrelated_score})"
            );
        }

        #[test]
        fn keyword_stuffed_wrong_answer_does_not_win_on_word_overlap_alone() {
            // A naive word-overlap-only scorer is exactly what this is
            // designed to resist: an answer that repeats ground-truth
            // keywords without actually answering correctly.
            let gt = "The Eiffel Tower is located in Paris, France.";
            let stuffed = "Paris France Eiffel Tower Paris France located located.";
            let correct = "The Eiffel Tower stands in Paris, France.";
            let stuffed_score = score_pair(gt, stuffed);
            let correct_score = score_pair(gt, correct);
            assert!(
                correct_score >= stuffed_score,
                "expected correct ({correct_score}) >= stuffed ({stuffed_score})"
            );
        }

        #[test]
        fn scores_vary_across_a_small_benchmark() {
            // Stage 2's own promotion check requires real variance across
            // a benchmark, a module that returns the same number for
            // everything is rejected. Sanity check that locally.
            let gt = "Water boils at 100 degrees Celsius at sea level.";
            let a = score_pair(gt, "Water boils at 100 degrees Celsius at sea level.");
            let b = score_pair(gt, "Water freezes at 0 degrees Celsius.");
            let c = score_pair(gt, "The stock market closed higher today.");
            assert!(a > b && b > c, "expected a ({a}) > b ({b}) > c ({c})");
        }

        #[test]
        fn handles_long_input_without_panicking() {
            let gt = "The answer is forty two.";
            let long_answer = "word ".repeat(5000); // well beyond MAX_WORDS
            let _ = score_pair(gt, &long_answer);
        }

        #[test]
        fn handles_non_ascii_and_emoji_without_panicking() {
            let gt = "The answer is forty two.";
            let weird = "🎉🎉🎉 답은 사십이입니다 émoji test 🚀";
            let _ = score_pair(gt, weird);
        }
    }
}

// ---------------------------------------------------------------------
// The required WASM export. This is the only function the node actually
// calls. Per docs.telegraphprotocol.com/docs/scoring/build-a-scoring-module,
// `rank_answer` receives six i32 params (ptr+len for each of question,
// ground_truth, miner_answer, in that exact order) and returns a single f32.
// ---------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn rank_answer(
    _q_ptr: i32,
    _q_len: i32,
    gt_ptr: i32,
    gt_len: i32,
    ma_ptr: i32,
    ma_len: i32,
) -> f32 {
    unsafe {
        let ground_truth = read_str(gt_ptr, gt_len);
        let miner_answer = read_str(ma_ptr, ma_len);
        scoring::score_pair(ground_truth, miner_answer)
    }
}
```

---

## `dwcs/src/prototype.ts`

```typescript
/**
 * TypeScript mirror of dwcs/rust-module/src/lib.rs's scoring logic.
 *
 * This is NOT the deployed module. The actual scoring module that gets
 * compiled to WASM and registered on Telegraph lives in
 * dwcs/rust-module/src/lib.rs. This file exists purely so metric weights
 * and thresholds can be prototyped and tuned quickly in TypeScript before
 * porting a change back into the Rust source, which is slower to iterate
 * on. If you change the logic here, port the equivalent change to
 * lib.rs, and vice versa, do not let them drift into two different
 * scoring behaviors.
 *
 * See PROJECT_SPEC.md Section 4.2 for the design rationale, and decision
 * D9 for why this is a deterministic multi-metric ensemble rather than an
 * LLM-judge ensemble.
 */

const STOPWORDS = new Set([
  "the", "a", "an", "is", "are", "was", "were", "of", "in", "on", "at", "to",
  "and", "or", "but", "this", "that", "it", "i", "you", "he", "she", "we", "they",
]);

const VARIANCE_DAMPING_THRESHOLD = 0.05;

function tokenize(s: string): string[] {
  return s.split(/\s+/).filter(Boolean);
}

function wordOverlap(answer: string[], truth: string[]): number {
  if (answer.length === 0) return 0;
  const truthLower = new Set(truth.map((w) => w.toLowerCase()));
  const matched = answer.filter((w) => truthLower.has(w.toLowerCase())).length;
  return matched / answer.length;
}

function stopwordWeightedOverlap(answer: string[], truth: string[]): number {
  if (answer.length === 0) return 0;
  const truthLower = new Set(truth.map((w) => w.toLowerCase()));
  let matchedWeight = 0;
  let totalWeight = 0;
  for (const w of answer) {
    const weight = STOPWORDS.has(w.toLowerCase()) ? 0.3 : 1.0;
    totalWeight += weight;
    if (truthLower.has(w.toLowerCase())) matchedWeight += weight;
  }
  return totalWeight === 0 ? 0 : matchedWeight / totalWeight;
}

function bigramJaccard(answer: string[], truth: string[]): number {
  const aBigrams = Math.max(0, answer.length - 1);
  const tBigrams = Math.max(0, truth.length - 1);
  if (aBigrams === 0 || tBigrams === 0) return 0;

  const used = new Array(tBigrams).fill(false);
  let matched = 0;
  for (let i = 0; i < aBigrams; i++) {
    for (let j = 0; j < tBigrams; j++) {
      if (used[j]) continue;
      if (
        answer[i].toLowerCase() === truth[j].toLowerCase() &&
        answer[i + 1].toLowerCase() === truth[j + 1].toLowerCase()
      ) {
        used[j] = true;
        matched++;
        break;
      }
    }
  }
  const union = aBigrams + tBigrams - matched;
  return union === 0 ? 0 : matched / union;
}

function lcsRatio(answer: string[], truth: string[]): number {
  const n = answer.length;
  const m = truth.length;
  if (n === 0 || m === 0) return 0;

  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      if (answer[i - 1].toLowerCase() === truth[j - 1].toLowerCase()) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }
  const lcsLen = dp[n][m];
  const denom = Math.max(n, m);
  return lcsLen / denom;
}

function mean(values: number[]): number {
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function variance(values: number[], avg: number): number {
  return mean(values.map((v) => (v - avg) ** 2));
}

function combine(metrics: number[]): number {
  const avg = mean(metrics);
  const v = variance(metrics, avg);
  const min = Math.min(...metrics);

  const clamp = (x: number) => Math.max(0, Math.min(1, x));

  if (v <= VARIANCE_DAMPING_THRESHOLD) {
    return clamp(avg);
  }
  return clamp(0.5 * avg + 0.5 * min);
}

export interface MetricBreakdown {
  wordOverlap: number;
  stopwordWeightedOverlap: number;
  bigramJaccard: number;
  lcsRatio: number;
  variance: number;
  finalScore: number;
}

/**
 * Same top-level behavior as the Rust module's score_pair: empty answer
 * scores exactly 0, exact match scores exactly 1, otherwise compute and
 * combine the four metrics. Returns the full breakdown for tuning
 * purposes, the Rust module only returns the final f32.
 */
export function scorePairWithBreakdown(groundTruth: string, minerAnswer: string): MetricBreakdown {
  if (minerAnswer.trim() === "") {
    return { wordOverlap: 0, stopwordWeightedOverlap: 0, bigramJaccard: 0, lcsRatio: 0, variance: 0, finalScore: 0 };
  }
  if (groundTruth.trim().toLowerCase() === minerAnswer.trim().toLowerCase()) {
    return { wordOverlap: 1, stopwordWeightedOverlap: 1, bigramJaccard: 1, lcsRatio: 1, variance: 0, finalScore: 1 };
  }

  const answerWords = tokenize(minerAnswer);
  const truthWords = tokenize(groundTruth);

  const metrics = [
    wordOverlap(answerWords, truthWords),
    stopwordWeightedOverlap(answerWords, truthWords),
    bigramJaccard(answerWords, truthWords),
    lcsRatio(answerWords, truthWords),
  ];

  const avg = mean(metrics);
  const v = variance(metrics, avg);

  return {
    wordOverlap: metrics[0],
    stopwordWeightedOverlap: metrics[1],
    bigramJaccard: metrics[2],
    lcsRatio: metrics[3],
    variance: v,
    finalScore: combine(metrics),
  };
}

export function scorePair(groundTruth: string, minerAnswer: string): number {
  return scorePairWithBreakdown(groundTruth, minerAnswer).finalScore;
}
```

---

## `dwcs/tests/prototype.test.ts`

```typescript
import { scorePair, scorePairWithBreakdown } from "../src/prototype";

describe("scorePair (TS prototype, mirrors dwcs/rust-module/src/lib.rs)", () => {
  it("scores an empty answer exactly 0", () => {
    expect(scorePair("Paris is the capital of France.", "")).toBe(0);
    expect(scorePair("Paris is the capital of France.", "   ")).toBe(0);
  });

  it("scores an exact match exactly 1", () => {
    expect(scorePair("Paris is the capital of France.", "Paris is the capital of France.")).toBe(1);
  });

  it("scores a correct answer above an unrelated one", () => {
    const gt = "Paris is the capital of France.";
    const good = scorePair(gt, "The capital of France is Paris.");
    const bad = scorePair(gt, "Bananas are yellow and grow on trees.");
    expect(good).toBeGreaterThan(bad);
  });

  it("recognizes a reworded correct answer better than an unrelated one", () => {
    const gt = "The mitochondria is the powerhouse of the cell.";
    const reworded = scorePair(gt, "Mitochondria act as the cell's powerhouse.");
    const unrelated = scorePair(gt, "Stock prices fell sharply on Tuesday.");
    expect(reworded).toBeGreaterThan(unrelated);
  });

  it("does not let keyword stuffing beat an actually correct answer", () => {
    const gt = "The Eiffel Tower is located in Paris, France.";
    const stuffed = scorePair(gt, "Paris France Eiffel Tower Paris France located located.");
    const correct = scorePair(gt, "The Eiffel Tower stands in Paris, France.");
    expect(correct).toBeGreaterThanOrEqual(stuffed);
  });

  it("exposes a full metric breakdown for tuning", () => {
    const breakdown = scorePairWithBreakdown(
      "The mitochondria is the powerhouse of the cell.",
      "Mitochondria act as the cell's powerhouse."
    );
    expect(breakdown).toHaveProperty("wordOverlap");
    expect(breakdown).toHaveProperty("bigramJaccard");
    expect(breakdown).toHaveProperty("lcsRatio");
    expect(breakdown).toHaveProperty("variance");
    expect(breakdown.finalScore).toBeGreaterThan(0);
  });
});
```

---

## `package.json`

```json
{
  "name": "telegraph-cassandra",
  "version": "0.0.0",
  "private": true,
  "description": "Telegraph Protocol hackathon submission: DWCS (Script Author track) and Sentinel (Application track).",
  "scripts": {
    "test": "jest",
    "test:dwcs": "jest dwcs/tests",
    "test:app": "jest app/tests",
    "typecheck": "tsc --noEmit",
    "check:open-questions": "bash scripts/check_open_questions.sh"
  },
  "devDependencies": {
    "@types/jest": "^29.5.0",
    "jest": "^29.7.0",
    "ts-jest": "^29.1.0",
    "typescript": "^5.5.0"
  },
  "dependencies": {
    "@x402/fetch": "^0.1.0",
    "@x402/evm": "^0.1.0"
  },
  "jest": {
    "preset": "ts-jest",
    "testEnvironment": "node"
  }
}
```

---

## `scripts/check_open_questions.sh`

```bash
#!/usr/bin/env bash
# Lists current open-question statuses from docs/OPEN_QUESTIONS.md. Most
# items are now resolved as of Aug 22, this is mainly useful to check
# items 5 (Miner attribution mechanics) and 7 (registration status),
# the two still genuinely open.

set -euo pipefail

echo "Current open question statuses:"
grep -A1 "^### " docs/OPEN_QUESTIONS.md | grep -E "^###|Answer:"
```

---

## `scripts/deploy_dwcs.sh`

```bash
#!/usr/bin/env bash
# Builds, checks, and prepares DWCS for registration on Telegraph.
# Does NOT submit the on-chain transaction itself, that's a deliberate
# manual step (registerWasm via integrate.telegraphprotocol.com or a
# direct contract call), registration costs a real transaction and should
# never happen from an unattended script.

set -euo pipefail

cd "$(dirname "$0")/../dwcs/rust-module"

echo "Running host-side unit tests first..."
cargo test

echo ""
echo "Building release WASM binary..."
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown

WASM_FILE="target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm"

if [ ! -f "$WASM_FILE" ]; then
  echo "Build did not produce $WASM_FILE, check cargo output above."
  exit 1
fi

echo ""
echo "Checking for zero imports (required, a non-zero count means this is not a valid Telegraph scoring module)..."
if ! command -v wasm-tools &> /dev/null; then
  echo "wasm-tools not found. Install it (cargo install wasm-tools) before proceeding."
  exit 1
fi

IMPORT_COUNT=$(wasm-tools print "$WASM_FILE" | grep -c '(import' || true)
echo "Import count: $IMPORT_COUNT"
if [ "$IMPORT_COUNT" -ne 0 ]; then
  echo "FAILED: expected 0 imports, got $IMPORT_COUNT. Do not register this binary."
  echo "Check you built wasm32-unknown-unknown, not wasm32-wasip1."
  exit 1
fi

echo ""
echo "Build OK. File: $(pwd)/$WASM_FILE"
echo ""
echo "Next steps (manual, not automated by this script):"
echo "  1. Test against Telegraph's go-tester harness, see rust-module/README.md."
echo "  2. Host this file at a public https:// or ipfs:// URL."
echo "  3. Register via https://integrate.telegraphprotocol.com, or call"
echo "     registerWasm(wasmHash, wasmUrl, intent) directly."
```

---

## `scripts/run_local_eval.sh`

```bash
#!/usr/bin/env bash
# Runs every local test: the real Rust module's own tests, the TS
# prototype's tests, and the application's tests. No live Telegraph calls,
# no on-chain interaction, safe to run any time.

set -euo pipefail

echo "=== Rust module tests (the real, deployable scoring logic) ==="
(cd "$(dirname "$0")/../dwcs/rust-module" && cargo test)

echo ""
echo "=== TS prototype tests (mirrors the Rust logic, for fast iteration) ==="
npm run test:dwcs

echo ""
echo "=== Application (Sentinel) tests ==="
npm run test:app

echo ""
echo "All local tests passed. This does not validate live behavior against"
echo "Telegraph's actual Stage 2 benchmark or real x402 payments, those"
echo "require the go-tester harness and a funded testnet wallet respectively."
```

---

## `tsconfig.json`

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "outDir": "dist"
  },
  "include": ["dwcs/src", "app/src", "dwcs/tests", "app/tests"]
}
```

---

