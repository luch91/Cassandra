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

