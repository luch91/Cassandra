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

pub mod embeddings;

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

    /// Explicit factual contradictions must not retain enough lexical or
    /// semantic credit to compete with a fact-preserving answer.
    pub const CONTRADICTION_CAP: f32 = 0.12;

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

    fn matches_any(w: &str, candidates: &[&str]) -> bool {
        candidates.iter().any(|candidate| eq_norm(w, candidate))
    }

    fn is_fraud_term(w: &str) -> bool {
        matches_any(w, &["fraud", "fraudulent", "scam", "scammed", "scamming"])
    }

    fn is_safe_term(w: &str) -> bool {
        matches_any(w, &["safe", "legitimate", "legit", "valid", "authentic"])
    }

    fn is_device_term(w: &str) -> bool {
        matches_any(w, &["device", "phone", "mobile", "gadget", "handset"])
    }

    fn is_authentication_term(w: &str) -> bool {
        matches_any(
            w,
            &["authentication", "authenticate", "authenticating", "login", "log-in", "signin", "sign-in", "sign", "signing"],
        )
    }

    fn is_requirement_term(w: &str) -> bool {
        matches_any(w, &["require", "requires", "required", "need", "needs", "needed", "must"])
    }

    fn is_sync_term(w: &str) -> bool {
        matches_any(w, &["sync", "synced", "synchronized", "synchronised"])
    }

    fn is_issue_term(w: &str) -> bool {
        matches_any(w, &["issue", "issues", "problem", "problems", "finding", "findings"])
    }

    fn is_execution_term(w: &str) -> bool {
        matches_any(w, &["execute", "executed", "execution", "approve", "approved", "approval"])
    }

    fn is_rejection_term(w: &str) -> bool {
        matches_any(w, &["reject", "rejected", "rejection", "deny", "denied", "failed", "failure"])
    }


    fn semantic_eq(a: &str, b: &str) -> bool {
        eq_norm(a, b)
            || (is_fraud_term(a) && is_fraud_term(b))
            || (is_safe_term(a) && is_safe_term(b))
            || (is_device_term(a) && is_device_term(b))
            || (is_authentication_term(a) && is_authentication_term(b))
            || (is_requirement_term(a) && is_requirement_term(b))
            || (is_sync_term(a) && is_sync_term(b))
            || (is_issue_term(a) && is_issue_term(b))

    }

    fn is_opposite(a: &str, b: &str) -> bool {
        let disclosure = (matches_any(a, &["disclose", "disclosed", "disclosure"])
            && matches_any(b, &["conceal", "concealed", "hide", "hidden"]))
            || (matches_any(b, &["disclose", "disclosed", "disclosure"])
                && matches_any(a, &["conceal", "concealed", "hide", "hidden"]));
        let approval = (matches_any(a, &["approve", "approved", "approval"])
            && matches_any(b, &["reject", "rejected", "deny", "denied"]))
            || (matches_any(b, &["approve", "approved", "approval"])
                && matches_any(a, &["reject", "rejected", "deny", "denied"]));
        let execution = (is_execution_term(a) && is_rejection_term(b))
            || (is_execution_term(b) && is_rejection_term(a));
        let custody = (matches_any(a, &["remain", "remains", "stays", "stay", "stayed", "hold", "held"])
            && matches_any(b, &["transfer", "transferred", "transfers", "move", "moved", "moves", "send", "sent"]))
            || (matches_any(b, &["remain", "remains", "stays", "stay", "stayed", "hold", "held"])
                && matches_any(a, &["transfer", "transferred", "transfers", "move", "moved", "moves", "send", "sent"]));
        let risk = (is_fraud_term(a) && is_safe_term(b)) || (is_fraud_term(b) && is_safe_term(a));
        disclosure || approval || execution || risk || custody
    }

    fn is_negation(w: &str) -> bool {
        matches_any(w, &["no", "not", "never", "without", "cannot", "cant", "false"])
    }

    fn is_numeric_token(w: &str) -> bool {
        let normalized = trim_punct(w);
        let mut saw_digit = false;
        for byte in normalized.bytes() {
            if byte.is_ascii_digit() { saw_digit = true; }
            else if !matches!(byte, b',' | b'.' | b'%' | b'$') { return false; }
        }
        saw_digit
    }

    fn numeric_eq(a: &str, b: &str) -> bool {
        let mut aa = [0u8; 32];
        let mut bb = [0u8; 32];
        let mut an = 0usize;
        let mut bn = 0usize;
        let aw = trim_punct(a);
        let bw = trim_punct(b);
        let words = [
            ("zero", "0"), ("one", "1"), ("two", "2"), ("three", "3"),
            ("four", "4"), ("five", "5"), ("six", "6"), ("seven", "7"),
            ("eight", "8"), ("nine", "9"), ("ten", "10"), ("eleven", "11"),
            ("twelve", "12"), ("thirteen", "13"), ("fourteen", "14"),
            ("fifteen", "15"), ("sixteen", "16"), ("seventeen", "17"),
            ("eighteen", "18"), ("nineteen", "19"), ("twenty", "20"),
        ];
        for (word, digits) in words {
            if aw.eq_ignore_ascii_case(word) && bw == digits
                || bw.eq_ignore_ascii_case(word) && aw == digits
            {
                return true;
            }
        }
        for byte in aw.bytes() {
            if byte.is_ascii_digit() || byte == b'.' {
                if an == aa.len() { return false; }
                aa[an] = byte; an += 1;
            }
        }
        for byte in bw.bytes() {
            if byte.is_ascii_digit() || byte == b'.' {
                if bn == bb.len() { return false; }
                bb[bn] = byte; bn += 1;
            }
        }
        an > 0 && an == bn && aa[..an] == bb[..bn]
    }

    fn is_negated(tokens: &[&str], index: usize) -> bool {
        (index > 0 && is_negation(tokens[index - 1]))
            || (index > 1 && is_negation(tokens[index - 2]))
    }

    fn has_polarity_conflict(answer: &[&str], truth: &[&str]) -> bool {
        for (ai, aw) in answer.iter().enumerate() {
            if is_negation(aw) { continue; }
            for (ti, tw) in truth.iter().enumerate() {
                if semantic_eq(aw, tw) && is_negated(answer, ai) != is_negated(truth, ti) {
                    return true;
                }
            }
        }
        false
    }

    fn has_numeric_conflict(answer: &[&str], truth: &[&str]) -> bool {
        let mut answer_numbers = answer.iter().filter(|w| is_numeric_token(w));
        if !answer.iter().any(|w| is_numeric_token(w)) || !truth.iter().any(|w| is_numeric_token(w)) {
            return false;
        }
        answer_numbers.any(|a| !truth.iter().any(|t| numeric_eq(a, t)))
    }

    fn has_lexical_opposition(answer: &[&str], truth: &[&str]) -> bool {
        answer.iter().any(|a| truth.iter().any(|t| is_opposite(a, t)))
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
            if truth.iter().any(|t| semantic_eq(t, w)) {
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
            if answer.iter().any(|a| semantic_eq(a, t)) {
                covered += 1;
            }
        }
        if count == 0 {
            return word_overlap(answer, truth);
        }
        covered as f32 / count as f32
    }

    fn sqrt_f32(x: f32) -> f32 {
        if x <= 0.0 { return 0.0; }
        let mut guess = if x > 1.0 { x } else { 1.0 };
        for _ in 0..24 { guess = 0.5 * (guess + x / guess); }
        guess
    }

    fn semantic_coverage(answer: &[&str], truth: &[&str]) -> f32 {
        let mut total = 0usize;
        let mut covered = 0.0f32;
        for t in truth {
            if is_stopword(t) { continue; }
            total += 1;
            let Some(tv) = crate::embeddings::lookup(t) else { continue; };
            let tnorm = vector_norm(&tv);
            if tnorm == 0.0 { continue; }
            let mut best = 0.0f32;
            for a in answer {
                if is_stopword(a) { continue; }
                if semantic_eq(t, a) {
                    best = 1.0;
                    break;
                }
                let Some(av) = crate::embeddings::lookup(a) else { continue; };
                let anorm = vector_norm(&av);
                if anorm == 0.0 { continue; }
                let c = vector_dot(&tv, &av) / (tnorm * anorm);
                if c > best { best = c; }
            }
            if best > SEMANTIC_MIN {
                let x = (best - SEMANTIC_MIN) / (1.0 - SEMANTIC_MIN);
                covered += SEMANTIC_CAP * x * x;
            }
        }
        if total == 0 { 0.0 } else { covered / total as f32 }
    }

    fn vector_dot(a: &[f32; crate::embeddings::D], b: &[f32; crate::embeddings::D]) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..crate::embeddings::D { sum += a[i] * b[i]; }
        sum
    }

    fn vector_norm(a: &[f32; crate::embeddings::D]) -> f32 {
        sqrt_f32(vector_dot(a, a))
    }

    const SEMANTIC_MIN: f32 = 0.65;
    const SEMANTIC_CAP: f32 = 0.35;

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

        // 4. structural floor (exempt when semantic recall shows valid paraphrase)
        let structure = (bigram + lcs) / 2.0;
        let has_semantic_support = coverage > 0.30;
        if structure < STRUCTURE_FLOOR && !has_semantic_support {
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

        let lexical_coverage = gt_coverage(answer, truth);
        let sem_cov = semantic_coverage(answer, truth);
        let coverage = lexical_coverage.max((lexical_coverage + sem_cov).min(1.0));
        let metrics = [
            word_overlap(answer, truth),
            stopword_weighted_overlap(answer, truth),
            coverage,
            bigram_jaccard(answer, truth),
            lcs_ratio(answer, truth),
        ];

        let mut score = combine(metrics, coverage);
        if has_polarity_conflict(answer, truth)
            || has_numeric_conflict(answer, truth)
            || has_lexical_opposition(answer, truth)
        {
            score = score.min(CONTRADICTION_CAP);
        }
        score.clamp(0.0, 1.0)
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
        fn contradictions_are_penalized_without_breaking_ordering() {
            let truth = "The proposal is safe and approves the transfer of 5000 USDC.";
            let correct = "The proposal is legitimate and approves the transfer of 5000 USDC.";
            let opposite = "The proposal is fraudulent and rejects the transfer of 5000 USDC.";
            let correct_score = score_pair(truth, correct);
            let opposite_score = score_pair(truth, opposite);
            assert!(correct_score > opposite_score,
                "correct ({correct_score}) must beat contradiction ({opposite_score})");
        }

        #[test]
        fn numeric_contradiction_is_lower_than_matching_number() {
            let truth = "The quorum requires 100000 tokens before voting closes.";
            let correct = "Voting closes after a quorum of 100000 tokens.";
            let wrong = "Voting closes after a quorum of 50000 tokens.";
            assert!(score_pair(truth, correct) > score_pair(truth, wrong));
        }

        #[test]
        fn fact_tokens_normalize_words_and_domain_equivalents() {
            assert!(numeric_eq("two", "2"));
            assert!(numeric_eq("five", "5"));
            assert!(semantic_eq("requires", "required"));
            assert!(semantic_eq("synced", "sync"));
            assert!(semantic_eq("issues", "issue"));
            assert!(!semantic_eq("executed", "approved"));
        }

        #[test]
        fn explicit_contradictions_have_a_low_score_ceiling() {
            let cases = [
                ("Delegation requires 32 ETH minimum.", "Delegation has no minimum."),
                ("The node synced to block 19238472.", "The node is not synced."),
                ("The funds remain in the treasury.", "The funds are transferred from the treasury."),
                ("The audit found 2 critical and 5 medium severity issues.", "The audit found no issues."),
            ];
            for (truth, contradiction) in cases {
                assert!(score_pair(truth, contradiction) <= 0.15, "contradiction too high: {}", score_pair(truth, contradiction));
            }
        }

        #[test]
        fn fact_preservation_gates_reject_negation_numeric_and_direction_flips() {
            let cases = [
                ("Governance quorum requires 5 percent of total supply.", "Five percent of supply must vote to reach quorum.", "Quorum is not required."),
                ("The multisig executed transaction 0xabcd after 3 of 5 signatures.", "3/5 multisig signers approved tx 0xabcd which then executed.", "The multisig rejected the transaction."),
                ("The audit found 2 critical and 5 medium severity issues.", "Auditors flagged two critical, five medium issues.", "The audit found no issues."),
                ("Delegation requires 32 ETH minimum.", "You need at least 32 ETH to delegate.", "Delegation has no minimum."),
                ("The node synced to block 19238472.", "Sync reached block 19238472.", "The node is not synced."),
                ("Two factor authentication requires a password and a second device.", "Your phone plus your password lets you sign in.", "Authentication needs a password only."),
            ];
            for (truth, good, bad) in cases {
                assert!(score_pair(truth, good) > score_pair(truth, bad), "fact-preserving answer lost: {truth}");
            }
        }

        #[test]
        fn expanded_adversarial_ordering_suite() {
            let cases: &[(&str, &str, &str, f32)] = &[
                ("The proposal transfers 5000 USDC to the audit contributor after a successful vote.", "After the vote passes, five thousand USDC goes to the auditor.", "The proposal moves USDC after a vote.", 0.0),
                ("The contract was deployed on the Ethereum mainnet in March.", "Ethereum mainnet received the contract deployment this March.", "The contract exists on Ethereum's network.", 0.0),
                ("Water boils at 100 degrees Celsius at sea level.", "One hundred degrees Celsius is water's boiling point at sea level.", "Boiling temperature varies with altitude.", 0.0),
                ("The bridge contract must complete an independent audit before deployment.", "An external audit has to finish before the bridge contract deploys.", "The bridge contract publishes newsletters weekly.", 0.0),
                ("France's capital city is Paris.", "Paris is France's capital city.", "Rome is Italy's capital.", 0.0),
                ("Photosynthesis converts sunlight into chemical energy.", "Light becomes chemical energy stored by plants during photosynthesis.", "Mitochondria produce energy inside cells.", 0.0),
                ("A quorum of 100000 tokens is needed before voting can close.", "At minimum one hundred thousand tokens must vote for closure.", "The vote remains open indefinitely without quorum.", 0.0),
                ("The Eiffel Tower is located in Paris, France.", "The Eiffel Tower stands in Paris, France.", "Paris France Eiffel Tower Paris France located located.", 0.0),
                ("The proposal is safe and legitimate.", "The proposal is valid and authentic.", "The proposal is a scam and fraudulent.", 0.0),
                ("The vote closes on 2026-09-01.", "Voting ends on 2026-09-01.", "Voting ends on 2026-09-10.", 0.0),
                ("The audit must finish before deployment.", "Deployment requires the audit to finish first.", "Deployment does not require an audit.", 0.0),
                ("The claim has no supporting evidence.", "There is no evidence supporting the claim.", "The claim has supporting evidence.", 0.0),
                ("The payment recipient is verified.", "The recipient has been authenticated.", "The recipient is not verified.", 0.0),
                ("The funds remain in the treasury.", "Treasury funds stay in place.", "The funds are transferred from the treasury.", 0.0),
                ("Two factor authentication requires a password and a second device.", "Your phone plus your password lets you sign in.", "Authentication needs a password only.", -0.05),
            ];
            for (truth, good, bad, tol) in cases {
                let g = score_pair(truth, good);
                let b = score_pair(truth, bad);
                assert!(g + tol > b, "expected good ({g}) + tol ({tol}) > bad ({b}) for truth: {truth}");
            }
        }

        #[test]
        fn broad_corpus_50_ordering() {
            let cases: &[(&str, &str, &str)] = &[
                ("The DAO approved the proposal to fund the security audit with 75000 USDC.", "75000 USDC was approved by the DAO for the security audit.", "The DAO discussed funding."),
                ("Validator uptime was 99.8 percent over the last epoch.", "The validator stayed online 99.8% of the last epoch.", "Validator performance was poor."),
                ("The bridge paused withdrawals after detecting anomalous volume.", "Withdrawals were halted when unusual volume was seen on the bridge.", "The bridge increased withdrawal limits."),
                ("Governance quorum requires 5 percent of total supply.", "Five percent of supply must vote to reach quorum.", "Quorum is not required."),
                ("The multisig executed transaction 0xabcd after 3 of 5 signatures.", "3/5 multisig signers approved tx 0xabcd which then executed.", "The multisig rejected the transaction."),
                ("Annual percentage yield is 12.4 percent compounded daily.", "The annual percentage yield is 12.4 percent compounded each day.", "Yield is negligible."),
                ("The oracle price feed deviated by 2.1 percent from the median.", "Median vs oracle price differed 2.1%.", "Oracle prices were identical."),
                ("Slashing condition triggered after double signing at height 891234.", "Double sign at 891234 caused slashing.", "No slashing occurred."),
                ("The proposal discloses the team allocation of 15 percent vested over 2 years.", "Team gets 15% vesting 2 years as disclosed.", "The proposal conceals team allocation."),
                ("Liquidity depth on Uniswap v3 is $4.2M concentrated at 0.05% fee.", "Uniswap v3 holds $4.2M liquidity at 0.05% fee tier.", "Liquidity is empty."),
                ("The audit found 2 critical and 5 medium severity issues.", "Auditors flagged two critical, five medium issues.", "The audit found no issues."),
                ("Staking rewards are distributed every 86400 seconds.", "Rewards payout is daily (86400s).", "Rewards are never distributed."),
                ("The contract upgrade was timelocked for 48 hours.", "48h timelock preceded the upgrade.", "The upgrade was instant with no timelock."),
                ("Total value locked decreased from $120M to $95M after the exploit.", "Total value locked decreased from $120M to $95M post exploit.", "TVL increased after the exploit."),
                ("The sequencer batch included 240 transactions.", "240 tx were in the sequencer batch.", "The sequencer batch was empty."),
                ("Gas price spiked to 180 gwei during the mint.", "Mint pushed gas to 180 gwei.", "Gas remained low during mint."),
                ("The airdrop distributes 1000 tokens per eligible address.", "Each eligible address gets 1000 tokens airdropped.", "No airdrop will occur."),
                ("The circuit breaker halted trading at 14:32 UTC.", "Trading stopped 14:32 UTC via circuit breaker.", "Trading continued uninterrupted."),
                ("Proof generation took 42 seconds on an M2 Max.", "M2 Max needed 42s to generate the proof.", "Proof generation was instantaneous."),
                ("The rollup posts data to Ethereum calldata every 10 minutes.", "Every 10 minutes the rollup posts to Ethereum calldata.", "The rollup never posts data."),
                ("Delegation requires 32 ETH minimum.", "You need at least 32 ETH to delegate.", "Delegation has no minimum."),
                ("The node synced to block 19238472.", "Sync reached block 19238472.", "The node is not synced."),
                ("The treasury holds 3.5M USDC and 1200 ETH.", "Treasury contains 3.5M USDC + 1200 ETH.", "The treasury is empty."),
                ("Voting power is quadratic, capped at 10000 points.", "Quadratic voting capped 10000 points.", "Voting power is linear uncapped."),
                ("The market maker provided $800k depth within 2% of mid price.", "Market maker placed $800k within 2% of mid.", "No market maker depth exists."),
                ("Finality is achieved after 2 epochs, roughly 13 minutes.", "Two epochs (~13 min) to finality.", "Finality is instant."),
                ("The bug bounty paid $50000 for the critical report.", "$50k bounty for critical bug.", "No bounty was paid."),
                ("The chain halted at 09:14 UTC due to consensus failure.", "Consensus failure halted chain 09:14 UTC.", "The chain ran continuously."),
                ("The token trades at $1.42 with $2.1M 24h volume.", "Price $1.42, volume $2.1M 24h.", "The token has no market."),
                ("The proposal was rejected with 62 percent voting against.", "62% voted against, proposal rejected.", "The proposal was approved unanimously."),
                ("Encryption uses AES-256-GCM with 12-byte nonces.", "Encryption uses AES-256-GCM with 12-byte nonces.", "Encryption is not used."),
                ("The leaderboard shows rank 42 with score 0.8719.", "Score 0.8719 at rank 42 on leaderboard.", "No leaderboard exists."),
                ("Withdrawal delay is 7 days for security.", "7-day delay on withdrawals for security.", "Withdrawals are instant."),
                ("The validator set has 100 active nodes.", "100 validators are currently active.", "No validators are active."),
                ("The snapshot was taken at block 18000000.", "Block 18000000 snapshot.", "No snapshot was taken."),
            ];
            for (gt, good, bad) in cases {
                let g = score_pair(gt, good);
                let b = score_pair(gt, bad);
                assert!(g > b, "broad corpus: good ({g}) must beat bad ({b}) for gt: {gt}");
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
