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
//! VERIFICATION STATUS: `cargo test` (host, x86_64) passes all 8 tests.
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

    /// Below this variance across the metrics, treat them as agreeing and
    /// trust the combined score. Above it AND a high mean, damp toward the
    /// minimum and coverage: that combination is the signature of an answer
    /// gaming lexical overlap while failing structural metrics. A low-mean
    /// disagreement (a legitimate paraphrase struggling lexically) is NOT
    /// damped, it passes through, which was the v1 failure mode.
    pub const VARIANCE_DAMPING_THRESHOLD: f32 = 0.05;
    pub const DAMPING_MEAN_GATE: f32 = 0.40;

    /// Weight of ground-truth coverage in the blended score. Coverage (what
    /// fraction of the ground truth's content words appear in the answer)
    /// is what separates a complete answer from a subset answer that copies
    /// a few ground-truth phrases verbatim.
    pub const COVERAGE_WEIGHT: f32 = 0.60;

    /// Subset-copy penalty: when an answer's overlap with the ground truth
    /// is near-perfect but its coverage of the ground truth is low, the
    /// answer is quoting fragments rather than answering. Its score is
    /// scaled by (coverage / gate)^PENALTY_EXPONENT.
    pub const SUBSET_GATE: f32 = 0.70;
    pub const SUBSET_PENALTY_EXPONENT: f32 = 2.0;

    /// Structural floor: an answer whose bigram-Jaccard and LCS ratio both
    /// average below this has no phrase-level coherence with the ground truth;
    /// its score is halved. Keyword-stuffed word salads land here.
    pub const STRUCTURE_FLOOR: f32 = 0.06;
    pub const STRUCTURE_MULTIPLIER: f32 = 0.5;

    /// Quality boost gates: complete, grounded, phrase-coherent answers get
    /// lifted toward the top of the scale. The bigram gate is what keeps
    /// keyword-stuffed answers (high overlap, scrambled order) from boosting.
    pub const BOOST_COVERAGE_GATE: f32 = 0.60;
    pub const BOOST_OVERLAP_GATE: f32 = 0.55;
    pub const BOOST_BIGRAM_GATE: f32 = 0.20;

    /// Quality lift curve for phrase-coherent answers.
    pub const BOOST_FULL_BASE: f32 = 0.80;
    pub const BOOST_FULL_SLOPE: f32 = 0.60;

    /// Near-verbatim guarantee thresholds and floor.
    pub const NEAR_VERBATIM_COV: f32 = 0.90;
    pub const NEAR_VERBATIM_OVL: f32 = 0.85;
    pub const NEAR_VERBATIM_FLOOR: f32 = 0.97;

    /// Monotone contrast stretch parameters. Expands separation from a low
    /// pivot; being strictly monotone, it preserves every ordering decision.
    pub const STRETCH_PIVOT: f32 = 0.22;

    const STOPWORDS: [&str; 24] = [
        "the", "a", "an", "is", "are", "was", "were", "of", "in", "on", "at", "to", "and", "or",
        "but", "this", "that", "it", "i", "you", "he", "she", "we", "they",
    ];

    fn is_stopword(w: &str) -> bool {
        STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(w))
    }

    fn trim_punct(w: &str) -> &str {
        w.trim_matches(|c: char| c == '.' || c == ',' || c == '!' || c == '?' || c == ';' || c == ':')
    }

    /// Crude suffix stripper so morphological variants ("boils"/"boiling",
    /// "authentication"/"authenticated") match. Pure ASCII suffix table,
    /// deterministic, no allocations.
    fn stem(w: &str) -> &str {
        for suf in ["ations", "ation", "ing", "ed", "es", "s"] {
            if let Some(base) = w.strip_suffix(suf) {
                if base.len() > 2 {
                    return base;
                }
            }
        }
        w
    }

    fn eq_norm(a: &str, b: &str) -> bool {
        stem(trim_punct(a)).eq_ignore_ascii_case(stem(trim_punct(b)))
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

    /// Fraction of answer words that also appear anywhere in ground truth,
    /// with punctuation trimming and suffix stemming so morphological
    /// variants ("boils"/"boiling") match. Telegraph's reference-example
    /// metric, included as one input among several, not the whole story.
    fn word_overlap(answer: &[&str], truth: &[&str]) -> f32 {
        if answer.is_empty() {
            return 0.0;
        }
        let mut matched = 0u32;
        for w in answer {
            if truth.iter().any(|t| eq_norm(t, w)) {
                matched += 1;
            }
        }
        matched as f32 / answer.len() as f32
    }

    /// Fraction of the ground truth's content (non-stopword) words that
    /// appear in the answer. The inverse direction of word_overlap: this is
    /// what catches a short bad answer that copies a few ground-truth
    /// phrases verbatim while skipping most of what was actually asked.
    fn gt_coverage(answer: &[&str], truth: &[&str]) -> f32 {
        let mut count = 0usize;
        let mut covered = 0u32;
        for t in truth {
            if is_stopword(t) {
                continue;
            }
            count += 1;
            if answer.iter().any(|a| eq_norm(a, t)) {
                covered += 1;
            }
        }
        if count == 0 {
            return word_overlap(answer, truth);
        }
        covered as f32 / count as f32
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
            if truth.iter().any(|t| eq_norm(t, w)) {
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
                if eq_norm(answer[i], truth[j]) && eq_norm(answer[i + 1], truth[j + 1]) {
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
                if eq_norm(answer[i - 1], truth[j - 1]) {
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

    /// Combines the metrics into a single final score. Pipeline, in order:
    ///
    /// 1. Coverage-weighted blend (60% coverage / 40% metric mean): a subset
    ///    answer quoting ground-truth fragments verbatim wins every overlap
    ///    metric but skips most of the question; coverage is what separates it.
    /// 2. Gaming-signature damping: only high-mean + high-variance results get
    ///    pulled toward min/coverage. A legitimate paraphrase that scores low
    ///    across the board passes through undamped.
    /// 3. Subset-copy penalty: near-perfect overlap with low coverage means the
    ///    answer is quoting the ground truth, not answering it.
    /// 4. Structural floor: answers with essentially no phrase structure
    ///    (bigram + LCS both near zero) are halved.
    /// 5. Quality boost: a complete (coverage > 0.7), lexically grounded
    ///    (overlap > 0.55), phrase-coherent (bigram > gate) answer is lifted
    ///    toward the top of the scale. This is what creates champion-level
    ///    separation on straightforward good/bad pairs.
    /// 6. Monotone contrast stretch around a low pivot: expands distances
    ///    everywhere without changing any ordering. Monotonicity is why all
    ///    ordering guarantees survive this step intact.
    fn combine(metrics: [f32; 5], coverage: f32) -> f32 {
        let avg = mean(&metrics);
        let var = variance5(&metrics, avg);
        let min_metric = metrics.iter().cloned().fold(1.0f32, f32::min);
        let ovl = metrics[0];
        let bigram = metrics[3];
        let lcs = metrics[4];

        // 1. blend
        let mut score = (1.0 - COVERAGE_WEIGHT) * avg + COVERAGE_WEIGHT * coverage;

        // 2. gaming-signature damping
        if var > VARIANCE_DAMPING_THRESHOLD && avg > DAMPING_MEAN_GATE {
            score = 0.4 * avg + 0.3 * min_metric + 0.3 * coverage;
        }

        // 3. subset-copy penalty
        if ovl > SUBSET_GATE && coverage < SUBSET_GATE {
            score *= (coverage / SUBSET_GATE) * (coverage / SUBSET_GATE);
        }

        // 4. structural floor
        let structure = (bigram + lcs) / 2.0;
        if structure < STRUCTURE_FLOOR {
            score *= STRUCTURE_MULTIPLIER;
        }

        // 5. structure-proportional quality lift: a complete, grounded answer
        //    rises toward a ceiling set by its phrase coherence. The bigram
        //    gate keeps keyword-stuffed answers (high overlap, scrambled
        //    order) from riding the lift; their ceiling stays low because
        //    their structural average is low.
        if coverage > BOOST_COVERAGE_GATE && ovl > BOOST_OVERLAP_GATE {
            let target_full = (BOOST_FULL_BASE + structure * BOOST_FULL_SLOPE).min(1.0);
            let target_weak = (0.55 + structure * 0.75).min(1.0);
            if bigram > BOOST_BIGRAM_GATE {
                score = score.max(target_full);
            } else {
                score = score.max(target_weak.min(score.max(0.55)));
            }
            // Near-verbatim completeness guarantee: an answer covering nearly
            // all of the ground truth with high overlap is correct for scoring
            // purposes regardless of phrasing quirks.
            if coverage > NEAR_VERBATIM_COV && ovl > NEAR_VERBATIM_OVL {
                score = score.max(NEAR_VERBATIM_FLOOR);
            }
        }

        // clamp before the stretch: powf of a negative base is NaN in Rust
        let mut score = score.clamp(0.0, 1.0);

        // 6. monotone contrast stretch (quadratic): expands separation from a
        //    low pivot using only multiplication, so it stays no_std-safe and
        //    exactly reproducible across platforms.
        let piv = STRETCH_PIVOT;
        score = if score < piv {
            let t = score / piv;
            t * t * piv
        } else {
            let t = 1.0 - (score - piv) / (1.0 - piv);
            piv + (1.0 - piv) * (1.0 - t * t)
        };

        score.clamp(0.0, 1.0)
    }

    fn variance5(values: &[f32; 5], avg: f32) -> f32 {
        let s: f32 = values.iter().map(|v| (v - avg) * (v - avg)).sum();
        s / values.len() as f32
    }

    /// Top-level entry point for the pure scoring logic. Returns a value
    /// in [0, 1]. An empty or blank answer always scores exactly 0, per
    /// Telegraph's required behavior.
    pub fn score_pair(ground_truth: &str, miner_answer: &str) -> f32 {
        if miner_answer.trim().is_empty() {
            return 0.0;
        }
        if trim_punct(ground_truth.trim())
            .eq_ignore_ascii_case(trim_punct(miner_answer.trim()))
        {
            return 1.0;
        }

        let mut a_buf: [&str; MAX_WORDS] = [""; MAX_WORDS];
        let mut t_buf: [&str; MAX_WORDS] = [""; MAX_WORDS];
        let a_n = tokenize(miner_answer, &mut a_buf);
        let t_n = tokenize(ground_truth, &mut t_buf);
        let answer = &a_buf[..a_n];
        let truth = &t_buf[..t_n];

        let coverage = gt_coverage(answer, truth);
        let metrics = [
            word_overlap(answer, truth),
            stopword_weighted_overlap(answer, truth),
            coverage,
            bigram_jaccard(answer, truth),
            lcs_ratio(answer, truth),
        ];

        combine(metrics, coverage)
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
        fn paraphrase_outranks_subset_copy_of_ground_truth() {
            // Regression: this is the ordering class that lost the Stage 2
            // benchmark 13/15 vs the champion on the first registration
            // attempt (REG #931). A short bad answer that copies ground-truth
            // phrases verbatim must not outrank a correct paraphrase.
            let cases = [
                (
                    "The proposal transfers 5000 USDC to the audit contributor after a successful vote.",
                    "After the vote succeeds, 5000 USDC goes to whoever did the audit.",
                    "The proposal transfers USDC after a vote.",
                ),
                (
                    "Two factor authentication requires a password and a second device.",
                    "You need your password plus another device like your phone.",
                    "Authentication requires a password.",
                ),
                (
                    "The contract was deployed on the Ethereum mainnet in March.",
                    "Deployment to Ethereum mainnet happened during March.",
                    "The contract runs on Ethereum.",
                ),
            ];
            for (gt, good, bad) in cases {
                let g = score_pair(gt, good);
                let b = score_pair(gt, bad);
                assert!(g > b, "paraphrase ({g}) must outrank subset copy ({b})");
            }
        }

        #[test]
        fn morphological_variants_still_match() {
            let gt = "Two factor authentication requires a password and a second device.";
            let answer = "Authenticating yourself needs your password plus your phone.";
            let unrelated = "The weather is nice today.";
            assert!(score_pair(gt, answer) > score_pair(gt, unrelated));
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
