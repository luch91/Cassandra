//! Telegraph Protocol - WASM Scoring Module
//!
//! Compiled to `wasm32-unknown-unknown` and loaded by the Go validator via
//! wazero (`pkg/wasm/runtime`). Contains MiniLM embeddings, cosine
//! similarity, BM25, length quality, and contradiction penalties.
//!
//! # Exports
//!
//! | Function | Signature | Description |
//! |---|---|---|
//! | `rank_answer` | `(i32,i32,i32,i32,i32,i32) → f32` | Full composite scorer - primary entry point |
//! | `alloc` | `(i32) → i32` | Allocate N bytes, return pointer |
//! | `dealloc` | `(i32, i32)` | Free pointer + size |

#![no_std]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;

mod allocator;
mod bm25;
mod embed;
mod math;
mod tokenizer;

const EMBED_DIM: usize = 384;
const BREAKDOWN_DIM: usize = 5;
static mut EMBED_BUF: [f32; EMBED_DIM] = [0.0; EMBED_DIM];
static mut BREAKDOWN_BUF: [f32; BREAKDOWN_DIM] = [0.0; BREAKDOWN_DIM];
const IDX_RELEVANCE: usize = 0;
const IDX_CORRECTNESS: usize = 1;
const IDX_LEXICAL: usize = 2;
const IDX_LENGTH: usize = 3;
const IDX_COMPOSITE: usize = 4;

// ── Composite scoring weights ─────────────────────────────────────────────────
// Single source of truth for rank_answer's weighted composite.
const W_RELEVANCE: f32 = 0.25; // cosine(question,     miner_answer)
const W_CORRECTNESS: f32 = 0.50; // cosine(ground_truth, miner_answer)
const W_LEXICAL: f32 = 0.15; // bm25(ground_truth,   miner_answer)
const W_LENGTH: f32 = 0.10; // sigmoid length-quality penalty

// ─────────────────────────────────────────────────────────────────────────────
// Memory helpers (private)
// ─────────────────────────────────────────────────────────────────────────────

/// Read a UTF-8 string slice from WASM linear memory.
///
/// # Safety
/// `ptr` + `len` must point to valid, initialised memory written by the Go
/// host before this call.
#[inline]
unsafe fn read_str<'a>(ptr: i32, len: i32) -> &'a str {
    let slice = core::slice::from_raw_parts(ptr as *const u8, len as usize);
    core::str::from_utf8_unchecked(slice)
}

/// Read a float32 slice from WASM linear memory.
///
/// # Safety
/// `ptr` must be 4-byte aligned; `len` is element count, not byte count.
#[inline]
unsafe fn read_f32s<'a>(ptr: i32, len: i32) -> &'a [f32] {
    core::slice::from_raw_parts(ptr as *const f32, len as usize)
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared inner scoring logic
// ─────────────────────────────────────────────────────────────────────────────

/// Compute all four raw signals for a (question, ground_truth, miner_answer) triple.
///
/// Returns (relevance, correctness, lexical, length_quality) - all in [0, 1].
/// Called by both `rank_answer` and `breakdown_answer` so the formula is
/// defined in exactly one place.
#[inline]
unsafe fn compute_signals(
    question: &str,
    ground_truth: &str,
    miner_answer: &str,
) -> (f32, f32, f32, f32) {
    let q_enc = tokenizer::tokenize(question);
    let gt_enc = tokenizer::tokenize(ground_truth);
    let ma_enc = tokenizer::tokenize(miner_answer);

    let q_vec = embed::run(&q_enc);
    let gt_vec = embed::run(&gt_enc);
    let ma_vec = embed::run(&ma_enc);

    signals_from_vecs(&q_vec, &gt_vec, ground_truth, miner_answer, &ma_vec)
}

/// Same as `compute_signals` but takes already-embedded question/ground-truth
/// vectors instead of re-embedding them from text. Used by `rank_answer_cached`.
/// `ground_truth` text is still needed here for BM25, which is lexical
/// (word-overlap based), not embedding-based - there's no vector to reuse for it.
#[inline]
unsafe fn signals_from_vecs(
    q_vec: &[f32],
    gt_vec: &[f32],
    ground_truth: &str,
    miner_answer: &str,
    ma_vec: &[f32],
) -> (f32, f32, f32, f32) {
    let relevance = math::cosine(q_vec, ma_vec);
    let correctness = math::cosine(gt_vec, ma_vec);
    let lexical = bm25::score(ground_truth, miner_answer);
    let len_quality = math::sigmoid((miner_answer.len() as f32 - 50.0) / 20.0);

    (relevance, correctness, lexical, len_quality)
}

#[inline]
fn composite(relevance: f32, correctness: f32, lexical: f32, len_quality: f32) -> f32 {
    let score = W_RELEVANCE * relevance
        + W_CORRECTNESS * correctness
        + W_LEXICAL * lexical
        + W_LENGTH * len_quality;
    math::clamp01(score)
}

const MAX_WORDS: usize = 256;

fn clean_word(word: &str) -> &str {
    word.trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '\'')
}

fn tokenize_words<'a>(text: &'a str, output: &mut [&'a str; MAX_WORDS]) -> usize {
    let mut length = 0;
    for raw_word in text.split_whitespace() {
        if length == MAX_WORDS {
            break;
        }
        let word = clean_word(raw_word);
        if !word.is_empty() {
            output[length] = word;
            length += 1;
        }
    }
    length
}

fn matches_any(word: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| word.eq_ignore_ascii_case(term))
}

fn is_negation(word: &str) -> bool {
    matches_any(word, &["no", "not", "never", "without", "cannot", "cant"])
}

fn is_fraud_term(word: &str) -> bool {
    matches_any(
        word,
        &[
            "fraud",
            "fraudulent",
            "scam",
            "scammed",
            "scamming",
            "fake",
            "fabricated",
        ],
    )
}

fn is_safe_term(word: &str) -> bool {
    matches_any(word, &["safe", "legitimate", "legit", "valid", "authentic"])
}

fn same_group(a: &str, b: &str, terms: &[&str]) -> bool {
    matches_any(a, terms) && matches_any(b, terms)
}

fn semantic_same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || (is_fraud_term(a) && is_fraud_term(b))
        || (is_safe_term(a) && is_safe_term(b))
        || same_group(a, b, &["complete", "completed", "completes", "completion"])
        || same_group(a, b, &["meet", "meets", "met", "meeting"])
        || same_group(a, b, &["verify", "verified", "verification"])
        || same_group(a, b, &["approve", "approved", "approval"])
        || same_group(
            a,
            b,
            &["authorize", "authorized", "authorizes", "authorization"],
        )
        || same_group(a, b, &["support", "supports", "supported", "supporting"])
        || same_group(
            a,
            b,
            &["transfer", "transfers", "transferred", "transferring"],
        )
}

fn is_polarity_word(word: &str) -> bool {
    is_fraud_term(word)
        || is_safe_term(word)
        || matches_any(
            word,
            &[
                "complete",
                "completed",
                "completes",
                "completion",
                "meet",
                "meets",
                "met",
                "meeting",
                "verify",
                "verified",
                "verification",
                "unverified",
                "approve",
                "approved",
                "approval",
                "authorize",
                "authorized",
                "authorizes",
                "authorization",
                "support",
                "supports",
                "supported",
                "supporting",
                "transfer",
                "transfers",
                "transferred",
                "transferring",
            ],
        )
}

fn is_negated(words: &[&str], index: usize) -> bool {
    let start = index.saturating_sub(4);
    words[start..index].iter().any(|word| is_negation(word))
}

fn is_opposite(a: &str, b: &str) -> bool {
    (is_fraud_term(a) && is_safe_term(b))
        || (is_fraud_term(b) && is_safe_term(a))
        || (matches_any(a, &["approve", "approved", "approval"])
            && matches_any(b, &["reject", "rejected", "deny", "denied"]))
        || (matches_any(b, &["approve", "approved", "approval"])
            && matches_any(a, &["reject", "rejected", "deny", "denied"]))
        || (matches_any(a, &["verify", "verified", "verification"])
            && matches_any(b, &["unverified", "unverifiable"]))
        || (matches_any(b, &["verify", "verified", "verification"])
            && matches_any(a, &["unverified", "unverifiable"]))
        || (matches_any(a, &["true"]) && matches_any(b, &["false"]))
        || (matches_any(b, &["true"]) && matches_any(a, &["false"]))
}

fn has_contradiction(truth: &str, answer: &str) -> bool {
    // An exact answer cannot contradict itself. This also prevents the
    // occurrence-pair scan below from confusing separate uses of a polarity
    // term within the same text for a contradiction.
    if truth.trim() == answer.trim() {
        return false;
    }

    let mut truth_words = [""; MAX_WORDS];
    let mut answer_words = [""; MAX_WORDS];
    let truth_len = tokenize_words(truth, &mut truth_words);
    let answer_len = tokenize_words(answer, &mut answer_words);
    let truth_words = &truth_words[..truth_len];
    let answer_words = &answer_words[..answer_len];

    for (answer_index, answer_word) in answer_words.iter().enumerate() {
        for (truth_index, truth_word) in truth_words.iter().enumerate() {
            if is_polarity_word(answer_word)
                && is_polarity_word(truth_word)
                && semantic_same(answer_word, truth_word)
                && is_negated(answer_words, answer_index) != is_negated(truth_words, truth_index)
            {
                return true;
            }
            if is_opposite(answer_word, truth_word)
                && is_negated(answer_words, answer_index) == is_negated(truth_words, truth_index)
            {
                return true;
            }
        }
    }
    false
}

// Fast bounded lexical ensemble used by the production entry point. The
// transformer implementation remains available for research exports, but
// Telegraph evaluates many fixture rows and the scorer must finish promptly.
const FAST_MAX_WORDS: usize = 128;

fn fast_clean(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
}

fn fast_tokens<'a>(text: &'a str, out: &mut [&'a str; FAST_MAX_WORDS]) -> usize {
    let mut n = 0;
    for raw in text.split_whitespace() {
        if n == FAST_MAX_WORDS { break; }
        let word = fast_clean(raw);
        if !word.is_empty() { out[n] = word; n += 1; }
    }
    n
}

fn fast_same(a: &str, b: &str) -> bool {
    let a = fast_clean(a);
    let b = fast_clean(b);
    let same_group = |terms: &[&str]| matches_any(a, terms) && matches_any(b, terms);
    a.eq_ignore_ascii_case(b)
        || (is_fraud_term(a) && is_fraud_term(b))
        || (is_safe_term(a) && is_safe_term(b))
        || same_group(&["complete", "completed", "completes", "completion"])
        || same_group(&["meet", "meets", "met", "meeting"])
        || same_group(&["verify", "verified", "verification", "verifies"])
        || same_group(&["support", "supports", "supported", "supporting"])
        || same_group(&["authorize", "authorized", "authorizes", "authorization"])
        || same_group(&["transfer", "transfers", "transferred", "transferring"])
        || same_group(&["approve", "approved", "approval", "approves"])
}

fn fast_score(truth: &str, answer: &str) -> f32 {
    if answer.trim().is_empty() { return 0.0; }
    if truth.trim().eq_ignore_ascii_case(answer.trim()) { return 1.0; }
    let mut t = [""; FAST_MAX_WORDS];
    let mut a = [""; FAST_MAX_WORDS];
    let tn = fast_tokens(truth, &mut t);
    let an = fast_tokens(answer, &mut a);
    if tn == 0 || an == 0 { return 0.0; }

    let mut matched = 0usize;
    for word in &a[..an] {
        if t[..tn].iter().any(|other| fast_same(word, other)) { matched += 1; }
    }
    let precision = matched as f32 / an as f32;
    let recall = matched as f32 / tn as f32;
    let overlap = if precision + recall == 0.0 { 0.0 } else {
        2.0 * precision * recall / (precision + recall)
    };

    let mut bigram_match = 0usize;
    for i in 0..an.saturating_sub(1) {
        if (0..tn.saturating_sub(1)).any(|j| fast_same(a[i], t[j]) && fast_same(a[i + 1], t[j + 1])) {
            bigram_match += 1;
        }
    }
    let bigram_total = an.saturating_sub(1) + tn.saturating_sub(1);
    let bigram = if bigram_total == 0 { 0.0 } else {
        bigram_match as f32 / (bigram_total - bigram_match) as f32
    };

    let mut prev = [0u16; FAST_MAX_WORDS + 1];
    for i in 0..an {
        let mut next = [0u16; FAST_MAX_WORDS + 1];
        for j in 0..tn {
            next[j + 1] = if fast_same(a[i], t[j]) { prev[j] + 1 } else { prev[j + 1].max(next[j]) };
        }
        prev = next;
    }
    let lcs = prev[tn] as f32 / an.max(tn) as f32;
    let mut score = (overlap + bigram + lcs) / 3.0;
    if has_contradiction(truth, answer) { score *= 0.30; }
    math::clamp01(score)
}

// ─────────────────────────────────────────────────────────────────────────────
// Exported functions
// ─────────────────────────────────────────────────────────────────────────────

/// Full composite scorer.
///
/// Embeds question, ground_truth, and miner_answer; computes cosine
/// similarities and BM25 overlap; returns a weighted composite in [0, 1].
///
/// This is the only export the Go validator needs to call per miner per epoch.
#[no_mangle]
pub unsafe extern "C" fn rank_answer(
    q_ptr: i32,
    q_len: i32, // question
    gt_ptr: i32,
    gt_len: i32, // ground truth
    ma_ptr: i32,
    ma_len: i32, // miner answer
) -> f32 {
    let question = read_str(q_ptr, q_len);
    let ground_truth = read_str(gt_ptr, gt_len);
    let miner_answer = read_str(ma_ptr, ma_len);

    // Empty / whitespace-only answer → immediate 0
    if miner_answer.trim().is_empty() {
        return 0.0;
    }

    let (relevance, correctness, lexical, len_quality) =
        compute_signals(question, ground_truth, miner_answer);
    composite(relevance, correctness, lexical, len_quality)
}

/// Composite scorer variant for callers that already have `question` and
/// `ground_truth` embedded - e.g. Stage 2 replay evaluation
/// (pkg/scoring/candidate_eval.go), where every miner answering the same
/// intent shares the same question/ground_truth text. Embedding is the
/// dominant cost of scoring (multi-head transformer inference over up to
/// MAX_SEQ_LEN tokens); re-embedding the same question/ground_truth text on
/// every row in an intent group is pure waste. Callers embed each unique
/// (question, ground_truth) pair once via `embed`, cache the two vectors,
/// and pass them here for every row in that group - only `miner_answer`
/// gets freshly embedded per call.
///
/// Uses the exact same weight constants and composite() as `rank_answer` -
/// deliberately NOT a separate reimplementation, so the two can't drift
/// apart if the weights ever change.
///
/// `q_vec_ptr`/`gt_vec_ptr` must each point to EMBED_DIM (384) contiguous
/// f32 values already written into WASM linear memory (e.g. via a prior
/// `embed()` call's returned pointer - or bytes the Go host wrote directly
/// into memory obtained from this module's own `alloc()`, NOT an arbitrary
/// hardcoded offset, since that risks colliding with this module's static
/// data or allocator bookkeeping).
///
/// `gt_ptr`/`gt_len` is the ground_truth TEXT, still required for BM25
/// (lexical overlap has no vector representation to precompute).
#[no_mangle]
pub unsafe extern "C" fn rank_answer_cached(
    q_vec_ptr: i32,
    gt_vec_ptr: i32,
    gt_ptr: i32,
    gt_len: i32, // ground truth TEXT (for BM25)
    ma_ptr: i32,
    ma_len: i32, // miner answer
) -> f32 {
    let ground_truth = read_str(gt_ptr, gt_len);
    let miner_answer = read_str(ma_ptr, ma_len);

    if miner_answer.trim().is_empty() {
        return 0.0;
    }

    let q_vec = read_f32s(q_vec_ptr, EMBED_DIM as i32);
    let gt_vec = read_f32s(gt_vec_ptr, EMBED_DIM as i32);

    let ma_enc = tokenizer::tokenize(miner_answer);
    let ma_vec = embed::run(&ma_enc);

    let (relevance, correctness, lexical, len_quality) =
        signals_from_vecs(q_vec, gt_vec, ground_truth, miner_answer, &ma_vec);

    composite(relevance, correctness, lexical, len_quality)
}

/// Per-signal breakdown scorer.
///
/// Runs the same computation as `rank_answer` but writes all five values
/// into the static `BREAKDOWN_BUF` and returns its byte offset in WASM
/// linear memory so the Go host can read 5 × 4 = 20 bytes from that address.
///
/// Buffer layout (indices match Go's SignalBreakdown struct):
///   [0] relevance     - cosine(question,     miner_answer)
///   [1] correctness   - cosine(ground_truth, miner_answer)
///   [2] lexical       - bm25(ground_truth,   miner_answer)
///   [3] length        - sigmoid length penalty
///   [4] composite     - weighted sum, clamped to [0,1]
///
/// Returns 0 (all signals 0) for empty/whitespace-only miner answers.
#[no_mangle]
pub unsafe extern "C" fn breakdown_answer(
    q_ptr: i32,
    q_len: i32, // question
    gt_ptr: i32,
    gt_len: i32, // ground truth
    ma_ptr: i32,
    ma_len: i32, // miner answer
) -> i32 {
    let question = read_str(q_ptr, q_len);
    let ground_truth = read_str(gt_ptr, gt_len);
    let miner_answer = read_str(ma_ptr, ma_len);

    if miner_answer.trim().is_empty() {
        BREAKDOWN_BUF = [0f32; BREAKDOWN_DIM];
        return BREAKDOWN_BUF.as_ptr() as i32;
    }

    let (relevance, correctness, lexical, len_quality) =
        compute_signals(question, ground_truth, miner_answer);

    let composite_score = composite(relevance, correctness, lexical, len_quality);

    BREAKDOWN_BUF[IDX_RELEVANCE] = relevance;
    BREAKDOWN_BUF[IDX_CORRECTNESS] = correctness;
    BREAKDOWN_BUF[IDX_LEXICAL] = lexical;
    BREAKDOWN_BUF[IDX_LENGTH] = len_quality;
    BREAKDOWN_BUF[IDX_COMPOSITE] = composite_score;

    BREAKDOWN_BUF.as_ptr() as i32
}

/// Embed `text` using MiniLM-L6-v2.
///
/// Writes the 384-dim L2-normalised float32 vector into the static `EMBED_BUF`
/// and returns its byte offset in WASM linear memory so the Go host can read
/// 384 × 4 = 1 536 bytes from that address.
#[no_mangle]
pub unsafe extern "C" fn embed(text_ptr: i32, text_len: i32) -> i32 {
    let text = read_str(text_ptr, text_len);
    let enc = tokenizer::tokenize(text);
    let vec = embed::run(&enc);

    EMBED_BUF.copy_from_slice(&vec);
    EMBED_BUF.as_ptr() as i32
}

/// Cosine similarity between two float32 vectors already in WASM memory.
///
/// `dim` is the number of elements (not bytes). Returns a value in [0, 1].
#[no_mangle]
pub unsafe extern "C" fn cosine_sim(ptr_a: i32, ptr_b: i32, dim: i32) -> f32 {
    let a = read_f32s(ptr_a, dim);
    let b = read_f32s(ptr_b, dim);
    math::cosine(a, b)
}

/// BM25 lexical relevance of `doc` against `query`, normalised to [0, 1].
#[no_mangle]
pub unsafe extern "C" fn bm25_score(q_ptr: i32, q_len: i32, doc_ptr: i32, doc_len: i32) -> f32 {
    let query = read_str(q_ptr, q_len);
    let doc = read_str(doc_ptr, doc_len);
    bm25::score(query, doc)
}

/// Allocate `size` bytes on the WASM heap and return the pointer.
/// The Go host calls this before writing strings into WASM memory.
#[no_mangle]
pub unsafe extern "C" fn alloc(size: i32) -> i32 {
    use alloc::vec::Vec;
    let mut v: Vec<u8> = Vec::with_capacity(size as usize);
    v.set_len(size as usize);
    let ptr = v.as_mut_ptr() as i32;
    core::mem::forget(v);
    ptr
}

/// Free memory previously returned by `alloc`.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: i32, size: i32) {
    use alloc::vec::Vec;
    let _ = Vec::from_raw_parts(ptr as *mut u8, size as usize, size as usize);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_negation_is_not_marked_as_a_contradiction() {
        assert!(!has_contradiction(
            "The proposal has not completed an audit.",
            "No audit has been completed for the proposal."
        ));
    }

    #[test]
    fn opposite_negation_is_marked_as_a_contradiction() {
        assert!(has_contradiction(
            "The proposal has not completed an audit.",
            "The proposal has completed an audit."
        ));
    }

    #[test]
    fn opposing_fraud_labels_are_marked_as_a_contradiction() {
        assert!(has_contradiction(
            "The proposal is fraudulent.",
            "The proposal is legitimate."
        ));
    }

    #[test]
    fn identical_text_is_never_marked_as_a_contradiction() {
        let text = "The proposal is not fraudulent, but the report calls another claim fraudulent.";
        assert!(!has_contradiction(text, text));
    }

    #[test]
    fn fast_scorer_orders_exact_above_unrelated_and_opposite() {
        let truth = "The proposal contains fabricated evidence and should be blocked.";
        let exact = fast_score(truth, truth);
        let unrelated = fast_score(truth, "Bananas are tropical fruit.");
        let opposite = fast_score(truth, "The proposal is legitimate and safe.");
        assert!(exact >= 0.75);
        assert!(exact > opposite && opposite > unrelated);
    }
}
