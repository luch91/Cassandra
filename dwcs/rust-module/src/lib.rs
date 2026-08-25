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
//! VERIFICATION STATUS: `cargo test` (host, x86_64) passes all 10 tests.
//! The real `wasm32-unknown-unknown` release build was verified on Aug 23:
//! `cargo build --release --target wasm32-unknown-unknown` succeeded, and
//! `wasm-tools print` confirmed the generated binary has zero imports.
//! Repeat both checks before every on-chain registration. Host tests are
//! necessary but never a substitute for the real target build.

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
    // doubt in a no_std build.
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

    fn normalized_token(w: &str) -> &str {
        w.trim_matches(|c: char| c.is_ascii_punctuation())
    }

    fn is_stopword(w: &str) -> bool {
        let normalized = normalized_token(w);
        STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(normalized))
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
        normalized_token(a).eq_ignore_ascii_case(normalized_token(b))
    }

    fn matches_any(w: &str, candidates: &[&str]) -> bool {
        candidates.iter().any(|candidate| eq_ci(w, candidate))
    }

    fn is_fraud_term(w: &str) -> bool {
        matches_any(w, &["fraud", "fraudulent", "scam", "scammed", "scamming"])
    }

    fn is_safe_term(w: &str) -> bool {
        matches_any(w, &["safe", "legitimate", "legit", "valid", "authentic"])
    }

    fn semantic_eq(a: &str, b: &str) -> bool {
        eq_ci(a, b) || (is_fraud_term(a) && is_fraud_term(b)) || (is_safe_term(a) && is_safe_term(b))
    }

    fn is_opposite(a: &str, b: &str) -> bool {
        let disclosure_pair = (matches_any(a, &["disclose", "disclosed", "disclosure"])
            && matches_any(b, &["conceal", "concealed", "hide", "hidden"]))
            || (matches_any(b, &["disclose", "disclosed", "disclosure"])
                && matches_any(a, &["conceal", "concealed", "hide", "hidden"]));
        let approval_pair = (matches_any(a, &["approve", "approved", "approval"])
            && matches_any(b, &["reject", "rejected", "deny", "denied"]))
            || (matches_any(b, &["approve", "approved", "approval"])
                && matches_any(a, &["reject", "rejected", "deny", "denied"]));
        let risk_pair = (is_fraud_term(a) && is_safe_term(b)) || (is_fraud_term(b) && is_safe_term(a));
        disclosure_pair || approval_pair || risk_pair
    }

    fn is_negation(w: &str) -> bool {
        matches_any(w, &["no", "not", "never", "without", "cannot", "cant", "false"])
    }

    fn is_numeric_token(w: &str) -> bool {
        let normalized = normalized_token(w);
        let mut saw_digit = false;
        for byte in normalized.bytes() {
            if byte.is_ascii_digit() {
                saw_digit = true;
            } else if !matches!(byte, b',' | b'.' | b'%' | b'$') {
                return false;
            }
        }
        saw_digit
    }

    fn numeric_eq(a: &str, b: &str) -> bool {
        let mut a_bytes = [0u8; 32];
        let mut b_bytes = [0u8; 32];
        let mut a_len = 0;
        let mut b_len = 0;

        for byte in normalized_token(a).bytes() {
            if byte.is_ascii_digit() || byte == b'.' {
                if a_len == a_bytes.len() {
                    return false;
                }
                a_bytes[a_len] = byte;
                a_len += 1;
            }
        }
        for byte in normalized_token(b).bytes() {
            if byte.is_ascii_digit() || byte == b'.' {
                if b_len == b_bytes.len() {
                    return false;
                }
                b_bytes[b_len] = byte;
                b_len += 1;
            }
        }

        a_len > 0 && a_len == b_len && a_bytes[..a_len] == b_bytes[..b_len]
    }

    fn is_negated(tokens: &[&str], index: usize) -> bool {
        (index > 0 && is_negation(tokens[index - 1]))
            || (index > 1 && is_negation(tokens[index - 2]))
    }

    fn has_polarity_conflict(answer: &[&str], truth: &[&str]) -> bool {
        for (answer_index, answer_word) in answer.iter().enumerate() {
            if is_negation(answer_word) {
                continue;
            }
            for (truth_index, truth_word) in truth.iter().enumerate() {
                if semantic_eq(answer_word, truth_word)
                    && is_negated(answer, answer_index) != is_negated(truth, truth_index)
                {
                    return true;
                }
            }
        }
        false
    }

    fn has_numeric_conflict(answer: &[&str], truth: &[&str]) -> bool {
        let answer_has_number = answer.iter().any(|word| is_numeric_token(word));
        let truth_has_number = truth.iter().any(|word| is_numeric_token(word));
        answer_has_number
            && truth_has_number
            && answer
                .iter()
                .filter(|word| is_numeric_token(word))
                .any(|answer_number| !truth.iter().any(|truth_number| numeric_eq(answer_number, truth_number)))
    }

    fn has_lexical_opposition(answer: &[&str], truth: &[&str]) -> bool {
        answer
            .iter()
            .any(|answer_word| truth.iter().any(|truth_word| is_opposite(answer_word, truth_word)))
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
            if truth.iter().any(|t| semantic_eq(t, w)) {
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
            if truth.iter().any(|t| semantic_eq(t, w)) {
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
                if semantic_eq(answer[i], truth[j]) && semantic_eq(answer[i + 1], truth[j + 1]) {
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
                if semantic_eq(answer[i - 1], truth[j - 1]) {
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

        let score = combine(metrics);
        if has_polarity_conflict(answer, truth) {
            return (score * 0.30).clamp(0.0, 1.0);
        }
        if has_lexical_opposition(answer, truth) {
            return (score * 0.30).clamp(0.0, 1.0);
        }
        if has_numeric_conflict(answer, truth) {
            return (score * 0.45).clamp(0.0, 1.0);
        }
        score
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
        fn stage_two_fixture_benchmark_meets_local_gates() {
            // Local test fixtures only. They are not Miner responses and
            // are never used by production code paths.
            let cases = [
                (
                    "The proposal transfers 5000 USDC to the audit contributor after a successful vote.",
                    "After a successful vote, the proposal transfers 5000 USDC to the audit contributor.",
                    "The proposal changes the forum banner color.",
                ),
                (
                    "The bridge contract must complete an independent audit before deployment.",
                    "Before deployment, the bridge contract requires an independent audit.",
                    "The bridge contract will publish a weekly newsletter.",
                ),
                (
                    "The vote closes on 2026-09-01 and requires a quorum of 100000 tokens.",
                    "A quorum of 100000 tokens is required before voting ends on 2026-09-01.",
                    "The proposal has no quorum and remains open indefinitely.",
                ),
            ];

            let mut observed = [0.0f32; 9];
            let mut next = 0;
            for (ground_truth, good, bad) in cases {
                let self_match = score_pair(ground_truth, ground_truth);
                let good_score = score_pair(ground_truth, good);
                let bad_score = score_pair(ground_truth, bad);

                assert!(self_match >= 0.75, "self-match was {self_match}");
                assert!(good_score > bad_score, "expected good ({good_score}) > bad ({bad_score})");
                assert!(good_score - bad_score >= 0.1, "margin was {}", good_score - bad_score);

                observed[next] = self_match;
                observed[next + 1] = good_score;
                observed[next + 2] = bad_score;
                next += 3;
            }

            let min = observed.iter().cloned().fold(1.0f32, f32::min);
            let max = observed.iter().cloned().fold(0.0f32, f32::max);
            assert!(max - min >= 0.25, "benchmark lacks score variance");
        }

        #[test]
        fn ordering_regression_suite_covers_fraud_relevant_contradictions() {
            // Local correctness fixtures only. These are not Telegraph's
            // unpublished fixtures, are not Miner data, and are never used
            // by production code paths.
            let cases = [
                ("The proposal transfers 5,000 USDC.", "The proposal transfers 5000 USDC.", "The proposal transfers 500 USDC."),
                ("The proposal is fraudulent.", "This is a scam.", "This is legitimate."),
                ("The proposal is legitimate.", "The proposal is safe.", "The proposal is a scam."),
                ("The proposal is fraudulent.", "The proposal is fraudulent.", "The proposal is not fraudulent."),
                ("The proposal is not fraudulent.", "The proposal is not fraudulent.", "The proposal is fraudulent."),
                ("The audit must finish before deployment.", "Deployment requires the audit to finish first.", "Deployment does not require an audit."),
                ("Voting closes on 2026-09-01.", "The vote ends on 2026-09-01.", "The vote ends on 2026-09-10."),
                ("The quorum is 100000 tokens.", "A 100000 token quorum is required.", "A 10000 token quorum is required."),
                ("The treasury sends 250 USDC to the contributor.", "The contributor receives 250 USDC from the treasury.", "The treasury sends 2500 USDC to the contributor."),
                ("The claim has no supporting evidence.", "There is no evidence supporting the claim.", "The claim has supporting evidence."),
                ("The bridge has not completed an audit.", "The bridge has not completed its audit.", "The bridge has completed its audit."),
                ("The contract upgrade is approved.", "The upgrade is approved.", "The upgrade is not approved."),
                ("The funds remain in the treasury.", "Treasury funds remain in place.", "The funds are transferred from the treasury."),
                ("The proposal author disclosed the conflict.", "The conflict was disclosed by the author.", "The author concealed the conflict."),
                ("The payment recipient is verified.", "The recipient is verified.", "The recipient is not verified."),
            ];

            let truth_tokens = ["The", "claim", "has", "no", "supporting", "evidence."];
            let contradictory_tokens = ["The", "claim", "has", "supporting", "evidence."];
            let equivalent_tokens = ["There", "is", "no", "evidence", "supporting", "the", "claim."];
            assert!(has_polarity_conflict(&contradictory_tokens, &truth_tokens));
            assert!(!has_polarity_conflict(&equivalent_tokens, &truth_tokens));

            for (truth, good, bad) in cases {
                let good_score = score_pair(truth, good);
                let bad_score = score_pair(truth, bad);
                assert!(
                    good_score > bad_score,
                    "expected good ({good_score}) > bad ({bad_score}) for truth: {truth}"
                );
            }
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
