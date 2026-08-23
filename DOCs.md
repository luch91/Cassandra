# Project Cassandra
### Disagreement-Weighted Canonical Scoring for Telegraph Protocol
**Telegraph program, Season I, 2026 (Track 2: Script Authors + Track 3: Applications)**

Status: Pre-build spec, locked for execution
Last verified against source: Aug 21, 2026

---

## 0. How to read this document

This is the execution spec, not a pitch deck. Every claim about how Telegraph works is sourced below. Everything we could **not** verify from public docs is flagged explicitly in Section 9. Do not build against assumptions in that section without confirming in the Telegraph Discord first. That is the single biggest risk to this project: building against a guessed API surface.

---

## 1. Source material (what we actually pulled, verbatim facts only)

| # | Source | What it told us |
|---|--------|------------------|
| 1 | `program.telegraphprotocol.com` | Track structure, prize pool, dates, "300+ builders registered" |
| 2 | `program.telegraphprotocol.com/rules` | Judging criteria weights, guardrails, non-negotiable rules, prize breakdown |
| 3 | `program.telegraphprotocol.com/supported-intents` | Full 40-intent catalog, Tier A/B split, ground-truth methods per intent |
| 4 | `telegraphprotocol.com` (main site) | Protocol mechanics: Miners, Validators, Script Authors, on-chain settlement, "1,000+ standardized AI skills" index |
| 5 | `integrate.telegraphprotocol.com` | Developer console exists. "Connect API," "Submit WASM," "Consume Intelligence" as the three literal submission paths |
| 6 | `docs.telegraphprotocol.com` | JS-rendered app shell only, could not extract page-level technical content via fetch. **Needs direct browser access before build starts.** |
| 7 | Third-party writeup, *"Telegraph Wants to Be the Visa of Machine Intelligence"* (theopensourcepress.com, Jun 8 2026) | Independent confirmation of mechanism: stake-weighted median consensus, cryptographic ground-truth checks, script authors earn a share of a 20% emission pool proportional to validator reliance, and, critically, **scripts that drift or produce volatility relative to consensus get automatically discarded by the network**, while accurate ones get automatically promoted |

---

## 2. The mechanism we are exploiting (in plain terms)

**Updated Aug 22 with the confirmed technical reality, see decision D9.** Telegraph is not a marketplace where an agent picks a provider. An agent declares an **Intent** (a domain, e.g. `FRAUD_DETECTION`), a confidence threshold, and a deadline. Telegraph routes the request to a Miner; the Miner's answer is scored against a ground-truth answer by a WASM scoring module. Higher, more consistent scores mean more routed traffic, more USDC earned, stronger Miners attracted, and the network compounds.

The scoring module itself is a sandboxed, self-contained WASM binary. It receives exactly three plain-text strings, `question`, `ground_truth`, `miner_answer`, and must return a single `f32` between 0 and 1. Critically: **it has no network access, no filesystem access, and no shared state across calls.** It cannot call an external LLM API. It cannot look anything up. It is pure, deterministic computation over the three strings it's given, nothing else. Every module also has to clear a two-stage promotion process: Stage 1 structural checks (loads correctly, scores a blank answer as exactly 0, scores a correct answer above an unrelated one, doesn't crash on adversarial input), then Stage 2, where it has to beat the currently active "champion" module on a fixed benchmark of good/bad answer pairs, on margin, win-count, and self-match strength, or it never goes live.

Two structural facts make this program winnable in a non-cliche way:

**Fact A: the network's own reference example is a naive word-overlap scorer.**
Telegraph's own documentation ships a simple word-overlap scoring example explicitly framed as "a legitimate starting point you can build on." That is exactly the naive, easily-gamed approach most entrants will ship as-is. It fails on the exact case Telegraph's own testing guidance names directly: "a reworded version of the correct answer that says the same thing differently... a good scorer should still recognize this as correct." Naive word overlap does not reliably do that. Whoever builds something structurally better than word-matching, while staying inside the no-network sandbox, has a real, demonstrable, and legitimately hard-to-replicate edge.

**Fact B: disagreement-as-signal still applies, just implemented differently than originally planned.**
The original plan for DWCS assumed calling multiple LLM judges at scoring time. That's not possible inside the sandbox. The mechanism has been redesigned (D9) to compute several structurally diverse, purely algorithmic similarity signals over the same `(ground_truth, miner_answer)` pair, treat disagreement across those signals as a within-module confidence check, and fold everything into one final `f32`. The core idea, disagreement across diverse evaluators is itself useful information, survives the pivot. The implementation does not.

**Fact C: promotion is judged against a real, known benchmark, not vague "network volatility."**
The earlier assumption (from a third-party writeup) that "the network discards volatile scripts" turned out to be a simplification. The real mechanism is concrete and testable: Stage 2 compares `candidate_margin`, `candidate_wins`, and `worst_self_match` against the current champion's numbers on a fixed benchmark set. This is good news, it means we can replicate this exact benchmark methodology locally before ever registering on-chain, using the same self-match and variance checks Telegraph itself uses (see Section 7 acceptance criteria).

---

## 3. Why this specific team is positioned to win this, not just enter it

This is not a borrowed idea. It is a direct port of a mechanism the team has already shipped and had recognized by the GenLayer core team:

- **Penumbra**, a library of 20 Intelligent Contract primitives built explicitly around *"disagreement as measurement instrument."* GenLayer's Intelligent Contracts already solve the exact problem Telegraph's Script Author track is now opening up as a program category: how do you get trustworthy, non-gameable consensus out of non-deterministic LLM outputs? GenLayer's answer is optimistic democracy, where validators vote, and it's disagreement among validator outputs, not a single validator's confidence, that becomes the actual trust signal.
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
4. Do **not** spread across more than 3 intents for program 1. Depth beats breadth: the guardrail requires 3+ active Miners and 100+ real Track 3 requests *per intent* for prize eligibility, so concentrating demand matters more than covering many intents thinly.

---

## 5. Track 3 build: the application, closing the loop

### 5.1 Concept
An autonomous compliance/fraud-triage agent that pays for `FRAUD_DETECTION` (plus `CONTENT_VERIFICATION`/`AI_TEXT_DETECTION` if the secondary intent is added) inference via x402 on a defined document stream (candidates below), and takes a confidence-gated action when the result clears a threshold.

**Corrected Aug 22, see decision D10.** "On-chain action" here has two distinct layers, don't conflate them:
- **Layer 1, automatic and Telegraph-native:** every paid x402 request is itself an on-chain-settled payment. The response includes a `signal_hash`, independently verifiable via `GET /engine/v1/signal/{signal_hash}` and visible on `explorer.telegraphprotocol.com`. This alone satisfies "must use Telegraph miners" and gives us a real, auditable receipt for every scored proposal, no extra integration needed.
- **Layer 2, our own addition, still an open build decision:** an explicit "flag this proposal" action written to a governance contract. Telegraph does not provide this, it has to be our own contract call into whichever governance system the chosen document stream (Section 5.2) uses. This is a real scope item, not a documentation gap, decide and record the specific contract/interface once the document stream is finalized.

This satisfies, directly, two of the program's own named "High-Value Areas to Explore":
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
- By owning both the scoring script (Track 2) and a live consumer of it (Track 3), we are not hoping strangers generate our qualifying demand. We generate it ourselves, deterministically, before the Sep 7 deadline, and we can demonstrate the full flywheel (Miner, Script, Application, real usage, re-ranking) as a single coherent narrative in the submission. That narrative, proving the flywheel works end-to-end, is literally what the rules page states is "the purpose of this program."

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
| D11 | Adopt strict repo hygiene: no em-dashes anywhere, professional conventional commit messages with no AI-tool attribution, and gitignore every `.md` file except files literally named `README.md` | This is a program submission judges and other builders will actually read and clone. A clean, professional-looking repo history and file structure is part of the credibility argument alongside the technical one. Keeping internal planning docs (spec, decision log, task list, agent instructions) untracked and local avoids cluttering the public repo with process artifacts while still letting them exist and inform the build | Aug 22 |
| D3 | Disagreement is a signal about Miner output, never license for our script's own volatility | Source #7 confirms Telegraph's meta-layer discards drifting/volatile scripts automatically. Conflating "detecting Miner gaming" with "our script being noisy" would get us discarded by the very mechanism we're trying to exploit. | Letting ensemble variance directly set our published canonical score without a stability check, rejected, too risky given confirmed discard mechanism | Aug 21 |
| D4 | Build Track 2 (script) and Track 3 (application) together, single team, single narrative | Guardrail requires 3+ Miners and 100+ real requests per intent; owning both ends lets us generate qualifying demand deterministically rather than hoping for organic traffic within a 7-day Track 3 window | Submitting only the script and hoping other program apps generate demand for our intents, rejected, too much dependency risk with only ~10 days of runway before Track 1/2 closes | Aug 21 |
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

## 10. Execution timeline (compressed against actual program dates)

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
2. Our solution is a working port of a previously-shipped, externally-recognized consensus mechanism (GenLayer/Penumbra's disagreement-as-signal, Counsel's on-chain judged advocate agent), not a novel, unproven idea invented for this program. That's a credibility argument judges can verify.
3. We proved the flywheel the program exists to test: Miner-equivalent signal, ranked by our script, consumed by our own live application, generating real demand, as a single closed loop, which is the literal stated purpose of program 1 per the rules page.
