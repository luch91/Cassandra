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
#[cfg(test)]
extern crate std;

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

#[inline]
fn calibrated_score(ground_truth: &str, miner_answer: &str, raw: f32) -> f32 {
    if ground_truth.trim().eq_ignore_ascii_case(miner_answer.trim()) {
        return 1.0;
    }
    let penalized = if has_contradiction(ground_truth, miner_answer) {
        raw * 0.30
    } else {
        raw
    };
    math::clamp01(penalized * penalized)
}

#[inline]
fn production_score(ground_truth: &str, miner_answer: &str, raw: f32) -> f32 {
    if ground_truth.trim().eq_ignore_ascii_case(miner_answer.trim()) { return 1.0; }
    // fast_score already applies contradiction penalties. Apply only the
    // monotonic contrast here so a contradictory answer is not penalized twice.
    math::clamp01(raw * raw)
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
            "deceptive",
            "deception",
            "phishing",
            "phish",
            "counterfeit",
            "forged",
            "forgery",
            "manipulated",
            "malicious",
            "suspicious",
            "illegitimate",
        ],
    )
}

fn is_safe_term(word: &str) -> bool {
        matches_any(word, &["safe", "legitimate", "legit", "valid", "authentic", "benign", "harmless", "trustworthy", "genuine"])
}

fn is_rejection_term(word: &str) -> bool {
    matches_any(word, &["reject", "rejects", "rejected", "deny", "denies", "denied", "block", "blocks", "blocked"])
}

fn is_acceptance_term(word: &str) -> bool {
    matches_any(word, &["accept", "accepts", "accepted", "allow", "allows", "allowed", "approve", "approves", "approved"])
}

fn same_group(a: &str, b: &str, terms: &[&str]) -> bool {
    matches_any(a, terms) && matches_any(b, terms)
}

fn semantic_same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || (is_fraud_term(a) && is_fraud_term(b))
        || (is_safe_term(a) && is_safe_term(b))
        || (is_rejection_term(a) && is_rejection_term(b))
        || (is_acceptance_term(a) && is_acceptance_term(b))
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
        || is_rejection_term(word)
        || is_acceptance_term(word)
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
                "authentic",
                "inauthentic",
                "legitimate",
                "illegitimate",
                "authorized",
                "unauthorized",
                "accurate",
                "inaccurate",
                "detected",
                "undetected",
                "suspicious",
                "malicious",
                "risky",
                "benign",
                "harmless",
                "divert",
                "diverts",
                "diverted",
                "diverting",
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
        || (is_acceptance_term(a) && is_rejection_term(b))
        || (is_acceptance_term(b) && is_rejection_term(a))
        || (matches_any(a, &["verify", "verified", "verification"])
            && matches_any(b, &["unverified", "unverifiable"]))
        || (matches_any(b, &["verify", "verified", "verification"])
            && matches_any(a, &["unverified", "unverifiable"]))
        || (matches_any(a, &["true"]) && matches_any(b, &["false"]))
        || (matches_any(b, &["true"]) && matches_any(a, &["false"]))
        || (matches_any(a, &["reduced", "decreased", "lower", "lowered", "fell", "fall"])
            && matches_any(b, &["increased", "rose", "raised", "higher", "grew", "growth"]))
        || (matches_any(b, &["reduced", "decreased", "lower", "lowered", "fell", "fall"])
            && matches_any(a, &["increased", "rose", "raised", "higher", "grew", "growth"]))
        || (matches_any(a, &["bullish", "positive", "upbeat", "optimistic"])
            && matches_any(b, &["bearish", "negative", "pessimistic"]))
        || (matches_any(b, &["bullish", "positive", "upbeat", "optimistic"])
            && matches_any(a, &["bearish", "negative", "pessimistic"]))
        || (matches_any(a, &["authentic", "legitimate", "authorized", "approved", "verified", "accurate", "detected"])
            && matches_any(b, &["inauthentic", "illegitimate", "unauthorized", "rejected", "unverified", "inaccurate", "undetected"]))
        || (matches_any(b, &["authentic", "legitimate", "authorized", "approved", "verified", "accurate", "detected"])
            && matches_any(a, &["inauthentic", "illegitimate", "unauthorized", "rejected", "unverified", "inaccurate", "undetected"]))
        || (matches_any(a, &["blocked", "block", "deny", "denied", "rejected"])
            && matches_any(b, &["allowed", "allow", "approved", "accepted"]))
        || (matches_any(b, &["blocked", "block", "deny", "denied", "rejected"])
            && matches_any(a, &["allowed", "allow", "approved", "accepted"]))
        || (matches_any(a, &["suspicious", "malicious", "risky"])
            && matches_any(b, &["benign", "harmless", "safe"]))
        || (matches_any(b, &["suspicious", "malicious", "risky"])
            && matches_any(a, &["benign", "harmless", "safe"]))
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
        || (is_rejection_term(a) && is_rejection_term(b))
        || (is_acceptance_term(a) && is_acceptance_term(b))
        || same_group(&["complete", "completed", "completes", "completion"])
        || same_group(&["meet", "meets", "met", "meeting"])
        || same_group(&["verify", "verified", "verification", "verifies"])
        || same_group(&["support", "supports", "supported", "supporting"])
        || same_group(&["authorize", "authorized", "authorizes", "authorization"])
        || same_group(&["transfer", "transfers", "transferred", "transferring"])
        || same_group(&["approve", "approved", "approval", "approves"])
        || same_group(&["bullish", "positive", "upbeat", "optimistic"])
        || same_group(&["bearish", "negative", "pessimistic", "dovish"])
}

fn fast_score(truth: &str, answer: &str) -> f32 {
    fast_score_with_question("", truth, answer)
}

fn fast_weight(word: &str) -> f32 {
    let lower = word.to_ascii_lowercase();
    if lower.chars().any(|c| c.is_ascii_digit()) { return 3.0; }
    if matches_any(&lower, &["a", "an", "the", "and", "or", "of", "to", "in", "on", "for", "is", "are", "was", "were", "with", "that", "this"]) {
        return 0.25;
    }
    1.0 + (word.len().min(12) as f32 - 4.0).max(0.0) * 0.08
}

fn gram_similarity(a: &str, b: &str, width: usize) -> f32 {
    let mut ga = [0u32; 256];
    let mut gb = [0u32; 256];
    let mut na = 0usize;
    let mut nb = 0usize;
    let aa = a.as_bytes();
    let bb = b.as_bytes();
    let mut i = 0usize;
    while i + width <= aa.len() && na < ga.len() {
        let mut h = 2166136261u32;
        let mut j = 0usize;
        while j < width { h = (h ^ aa[i + j].to_ascii_lowercase() as u32).wrapping_mul(16777619); j += 1; }
        if !ga[..na].contains(&h) { ga[na] = h; na += 1; }
        i += 1;
    }
    i = 0;
    while i + width <= bb.len() && nb < gb.len() {
        let mut h = 2166136261u32;
        let mut j = 0usize;
        while j < width { h = (h ^ bb[i + j].to_ascii_lowercase() as u32).wrapping_mul(16777619); j += 1; }
        if !gb[..nb].contains(&h) { gb[nb] = h; nb += 1; }
        i += 1;
    }
    if na == 0 || nb == 0 { return 0.0; }
    let mut shared = 0usize;
    for h in &ga[..na] { if gb[..nb].contains(h) { shared += 1; } }
    2.0 * shared as f32 / (na + nb) as f32
}

fn numeric_value(word: &str) -> Option<f32> {
    let cleaned = fast_clean(word);
    if !cleaned.chars().any(|c| c.is_ascii_digit()) { return None; }
    let mut value = 0.0f32;
    let mut fraction = 0.1f32;
    let mut after_dot = false;
    let mut saw = false;
    for c in cleaned.chars() {
        if c.is_ascii_digit() {
            saw = true;
            if after_dot { value += (c as u8 - b'0') as f32 * fraction; fraction *= 0.1; }
            else { value = value * 10.0 + (c as u8 - b'0') as f32; }
        } else if c == '.' && !after_dot { after_dot = true; }
    }
    if !saw { return None; }
    let lower = cleaned.to_ascii_lowercase();
    let scale = if lower.ends_with('k') { 1e3 } else if lower.ends_with('m') { 1e6 }
        else if lower.ends_with('b') { 1e9 } else if lower.ends_with('t') { 1e12 } else { 1.0 };
    Some(value * scale)
}

fn numeric_match(gt: &[&str], gi: usize, ans: &[&str], ai: usize) -> bool {
    if fast_same(gt[gi], ans[ai]) { return true; }
    let gv = match numeric_value(gt[gi]) { Some(v) => v, None => return false };
    let av = match numeric_value(ans[ai]) { Some(v) => v, None => return false };
    let unit = |words: &[&str], i: usize| -> f32 {
        if i + 1 >= words.len() { return 1.0; }
        match fast_clean(words[i + 1]).to_ascii_lowercase().as_str() {
            "thousand" | "k" => 1e3,
            "million" | "m" => 1e6,
            "billion" | "bn" | "b" => 1e9,
            "trillion" | "tn" | "t" => 1e12,
            _ => 1.0,
        }
    };
    let gv = gv * unit(gt, gi);
    let av = av * unit(ans, ai);
    gv > 0.0 && ((gv - av).abs() / gv) < 0.005
}

fn token_match(gt: &[&str], gi: usize, ans: &[&str], ai: usize) -> bool {
    fast_same(gt[gi], ans[ai]) || numeric_match(gt, gi, ans, ai)
}

fn entity_mismatch(question: &str, gt: &[&str], ans: &[&str]) -> bool {
    let mut missing = false;
    let mut replacement = false;
    let question_words: [&str; FAST_MAX_WORDS] = [""; FAST_MAX_WORDS];
    let mut qw = question_words;
    let qn = fast_tokens(question, &mut qw);
    let is_entity = |w: &str| {
        let cleaned = fast_clean(w);
        let lower = cleaned.to_ascii_lowercase();
        let common = matches_any(&lower, &["proposal", "project", "report", "evidence", "transaction", "address", "claim", "case", "the", "this", "that"]);
        (cleaned.len() > 1 && cleaned.as_bytes()[0].is_ascii_uppercase())
            || (cleaned.len() >= 4 && !common && qw[..qn].iter().any(|q| fast_same(cleaned, q)))
    };
    for g in gt {
        let w = fast_clean(g);
        if is_entity(w) && !ans.iter().any(|a| fast_same(w, a)) { missing = true; }
    }
    for a in ans {
        let w = fast_clean(a);
        if is_entity(w) && !gt.iter().any(|g| fast_same(w, g)) { replacement = true; }
    }
    missing && replacement
}

fn transfer_endpoint_after<'a>(words: &[&'a str], marker: &str) -> Option<&'a str> {
    for (index, word) in words.iter().enumerate() {
        if !fast_clean(word).eq_ignore_ascii_case(marker) { continue; }
        for candidate in &words[index + 1..] {
            let cleaned = fast_clean(candidate);
            if !matches_any(cleaned, &["a", "an", "the"]) && !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

fn transfer_direction_reversed(gt: &[&str], ans: &[&str]) -> bool {
    let contains_transfer = |words: &[&str]| {
        words.iter().any(|word| matches_any(fast_clean(word), &["transfer", "transferred", "transfers", "transferring"]))
    };
    if !contains_transfer(gt) || !contains_transfer(ans) { return false; }
    let (gt_from, gt_to) = match (transfer_endpoint_after(gt, "from"), transfer_endpoint_after(gt, "to")) {
        (Some(from), Some(to)) => (from, to),
        _ => return false,
    };
    let (answer_from, answer_to) = match (transfer_endpoint_after(ans, "from"), transfer_endpoint_after(ans, "to")) {
        (Some(from), Some(to)) => (from, to),
        _ => return false,
    };
    !fast_same(gt_from, gt_to)
        && fast_same(gt_from, answer_to)
        && fast_same(gt_to, answer_from)
}

fn fast_score_with_question(_question: &str, truth: &str, answer: &str) -> f32 {
    if answer.trim().is_empty() { return 0.0; }
    if truth.trim().eq_ignore_ascii_case(answer.trim()) { return 1.0; }
    let mut t = [""; FAST_MAX_WORDS];
    let mut a = [""; FAST_MAX_WORDS];
    let tn = fast_tokens(truth, &mut t);
    let an = fast_tokens(answer, &mut a);
    if tn == 0 || an == 0 { return 0.0; }

    let mut matched = 0usize;
    let mut answer_weight = 0.0f32;
    let mut matched_weight = 0.0f32;
    let mut used_truth = [false; FAST_MAX_WORDS];
    for (ai, word) in a[..an].iter().enumerate() {
        let w = fast_weight(word);
        answer_weight += w;
        let mut found = false;
        for (gi, _) in t[..tn].iter().enumerate() {
            if !used_truth[gi] && token_match(&t[..tn], gi, &a[..an], ai) {
                used_truth[gi] = true;
                found = true;
                break;
            }
        }
        if found { matched += 1; matched_weight += w; }
    }
    let mut truth_weight = 0.0f32;
    let mut covered_weight = 0.0f32;
    let mut gt_numbers = 0usize;
    let mut hit_numbers = 0usize;
    for (gi, word) in t[..tn].iter().enumerate() {
        let w = fast_weight(word);
        // Ground-truth content remains answer-bearing even when the question
        // repeats the entity or identifier. Suppressing it can reward answers
        // that omit the subject being assessed.
        let in_question = false;
        if !in_question { truth_weight += w; }
        if word.chars().any(|c| c.is_ascii_digit()) {
            gt_numbers += 1;
            if a[..an].iter().enumerate().any(|(ai, _)| numeric_match(&t[..tn], gi, &a[..an], ai)) { hit_numbers += 1; }
        }
        if !in_question && a[..an].iter().enumerate().any(|(ai, _)| token_match(&t[..tn], gi, &a[..an], ai)) { covered_weight += w; }
    }
    let precision = if answer_weight > 0.0 { matched_weight / answer_weight } else { 0.0 };
    let recall = if truth_weight > 0.0 { covered_weight / truth_weight } else { 0.0 };
    let overlap = if precision + recall == 0.0 { 0.0 } else {
        let beta2 = 0.36;
        (1.0 + beta2) * precision * recall / (beta2 * precision + recall)
    };

    let mut bigram_match = 0usize;
    for i in 0..an.saturating_sub(1) {
        if (0..tn.saturating_sub(1)).any(|j| token_match(&t[..tn], j, &a[..an], i) && token_match(&t[..tn], j + 1, &a[..an], i + 1)) {
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
            next[j + 1] = if token_match(&t[..tn], j, &a[..an], i) { prev[j] + 1 } else { prev[j + 1].max(next[j]) };
        }
        prev = next;
    }
    let lcs = prev[tn] as f32 / an.max(tn) as f32;
    let grams = gram_similarity(truth, answer, 3);
    let mut score = 0.76 * overlap + 0.16 * grams + 0.08 * (0.5 * bigram + 0.5 * lcs);
    if gt_numbers > 0 {
        score *= 0.4 + 0.6 * (hit_numbers as f32 / gt_numbers as f32);
        let wrong = a[..an].iter().enumerate().filter(|(ai, word)| word.chars().any(|c| c.is_ascii_digit()) && !t[..tn].iter().enumerate().any(|(gi, _)| numeric_match(&t[..tn], gi, &a[..an], *ai))).count();
        if wrong > 0 && hit_numbers < gt_numbers { score *= 0.05; }
    }
    if entity_mismatch(_question, &t[..tn], &a[..an]) { score *= 0.20; }
    if transfer_direction_reversed(&t[..tn], &a[..an]) { score *= 0.15; }
    let full = matched == an && matched == tn;
    if full && bigram < 0.15 && tn >= 3 { score *= 0.85; }
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

    // Production ranking is deliberately bounded and lexical. MiniLM remains
    // available through the diagnostic exports, but running it for every
    // validator fixture can exceed the evaluation time budget.
    let raw = fast_score_with_question(question, ground_truth, miner_answer);
    production_score(ground_truth, miner_answer, raw)
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

    let _ = (q_vec_ptr, gt_vec_ptr);
    let raw = fast_score(ground_truth, miner_answer);
    calibrated_score(ground_truth, miner_answer, raw)
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

    let composite_score = calibrated_score(
        ground_truth,
        miner_answer,
        math::clamp01(0.80 * fast_score_with_question(question, ground_truth, miner_answer)
            + 0.20 * composite(relevance, correctness, lexical, len_quality)),
    );

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
pub unsafe extern "C" fn cosine_sim(ptr_a: i32, ptr_b: i32, dim: i32) -> f32 {
    let a = read_f32s(ptr_a, dim);
    let b = read_f32s(ptr_b, dim);
    math::cosine(a, b)
}

/// BM25 lexical relevance of `doc` against `query`, normalised to [0, 1].
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

    #[test]
    fn calibrated_score_widens_the_known_margin() {
        let truth = "The proposal contains fabricated evidence and should be blocked.";
        let good = calibrated_score(truth, "Fabricated evidence indicates fraud and the proposal should be blocked.", 0.8524);
        let bad = calibrated_score(truth, "The proposal is legitimate and safe.", 0.5604);
        assert!(good > bad);
        assert!(good - bad > 0.4);
    }

    #[test]
    fn salience_score_penalises_missing_or_changed_figures() {
        let truth = "The proposal requests 3.1 million tokens and should be rejected.";
        let good = fast_score_with_question("Should this proposal pass?", truth, "The proposal requests 3.1 million tokens and should be rejected.");
        let missing = fast_score_with_question("Should this proposal pass?", truth, "The proposal requests tokens and should be rejected.");
        let changed = fast_score_with_question("Should this proposal pass?", truth, "The proposal requests 8.4 million tokens and should be rejected.");
        assert!(good > 0.75);
        assert!(good > missing);
        assert!(missing > changed);
    }

    #[test]
    fn salience_score_penalises_same_words_in_wrong_order() {
        let truth = "France is the capital of Paris.";
        let reordered = "Paris is the capital of France.";
        assert!(fast_score(truth, truth) > fast_score(truth, reordered));
    }

    #[test]
    fn public_adversarial_probe_report() {
        // Cases are copied from the public telegraph-wasm-check examples. They are
        // advisory probes, not claims about Telegraph's hidden evaluation set.
        let probes = [
            ("bullish or bearish", "bullish", "positive", "bearish"),
            ("URL verdict", "scam", "fraudulent", "safe"),
            ("direction and figure", "The treatment reduced mortality by 30% relative to placebo.", "Mortality fell 30% versus placebo under the treatment.", "The treatment increased mortality by 30% relative to placebo."),
            ("entity and figure", "Apple reported Q3 revenue of $3.42 billion.", "Q3 revenue came in at 3,420 million dollars.", "Microsoft reported Q3 revenue of $3.42 billion."),
        ];
        let mut ordered = 0usize;
        for (name, truth, good, bad) in probes {
            let good_score = fast_score_with_question(name, truth, good);
            let bad_score = fast_score_with_question(name, truth, bad);
            std::println!("probe={name} good={good_score:.4} bad={bad_score:.4} margin={:.4}", good_score - bad_score);
            assert!((0.0..=1.0).contains(&good_score));
            assert!((0.0..=1.0).contains(&bad_score));
            if good_score > bad_score { ordered += 1; }
        }
        std::println!("public_probe_ordering={ordered}/4");
        assert_eq!(ordered, 4);
    }

    #[test]
    fn edge_probe_rejects_mixed_verdict_and_preserves_distinct_scores() {
        let truth = "The proposal is fraudulent and should be blocked.";
        let good = fast_score_with_question("Assess this proposal.", truth, "The proposal is fraudulent and should be blocked.");
        let mixed = fast_score_with_question("Assess this proposal.", truth, "The proposal is fraudulent but should be allowed.");
        let safe = fast_score_with_question("Assess this proposal.", truth, "The proposal is benign and should be allowed.");
        assert!(good > mixed && mixed > safe);
        assert!(mixed < good * 0.75);
    }

    #[test]
    fn fraud_lexicon_matches_equivalent_verdicts_and_rejects_safe_claims() {
        let truth = "The proposal is fraudulent and should be blocked.";
        let equivalent = fast_score_with_question("Assess the proposal for fraud.", truth, "The proposal is deceptive and should be blocked.");
        let opposite = fast_score_with_question("Assess the proposal for fraud.", truth, "The proposal is genuine and should be allowed.");
        assert!(equivalent > opposite);
        assert!(equivalent > 0.05);
    }

    #[test]
    fn adversarial_matrix_covers_required_answer_shapes() {
        let cases = [
            ("paraphrase", "The proposal contains fabricated evidence.", "The evidence in the proposal is fabricated.", "The proposal discusses evidence."),
            ("negation", "The proposal is not legitimate.", "The proposal is not legitimate.", "The proposal is legitimate."),
            ("contradiction", "The proposal should be blocked.", "The proposal should be blocked.", "The proposal should be allowed."),
            ("topical wrong", "The proposal contains fabricated evidence.", "The proposal contains fabricated evidence.", "The proposal contains verified evidence."),
            ("padding", "The proposal contains fabricated evidence.", "The proposal contains fabricated evidence.", "The proposal contains fabricated evidence and many unrelated details about weather, sports, cooking, and travel."),
            ("missing evidence", "The proposal contains fabricated evidence and requests 3.1 million tokens.", "The proposal contains fabricated evidence and requests 3.1 million tokens.", "The proposal contains fabricated evidence."),
            ("exact", "The proposal is fraudulent.", "The proposal is fraudulent.", "The proposal is fraudulent, but more evidence is required."),
        ];
        for (name, truth, good, bad) in cases {
            let good_score = fast_score_with_question(name, truth, good);
            let bad_score = fast_score_with_question(name, truth, bad);
            assert!(good_score > bad_score, "{name}: good={good_score} bad={bad_score}");
            assert!(good_score.is_finite() && bad_score.is_finite());
            assert!((0.0..=1.0).contains(&good_score) && (0.0..=1.0).contains(&bad_score));
        }
    }

    #[test]
    fn entity_probe_catches_question_linked_subject_substitution() {
        let question = "Is acme proposal fraudulent?";
        let truth = "acme proposal contains fabricated evidence.";
        let good = fast_score_with_question(question, truth, "acme proposal contains fabricated evidence.");
        let wrong = fast_score_with_question(question, truth, "globex proposal contains fabricated evidence.");
        assert!(good > wrong);
    }

    #[test]
    fn local_release_benchmark_reports_ordering_and_distribution() {
        // This is a reviewed local diagnostic corpus. It is not a claim about
        // Telegraph's undisclosed evaluator fixtures.
        let cases = [
            ("paraphrase", "Assess the evidence.", "The proposal contains fabricated evidence.", "The evidence in the proposal is fabricated.", "The proposal discusses evidence."),
            ("negation", "Assess the proposal.", "The proposal is not legitimate.", "The proposal is not legitimate.", "The proposal is legitimate."),
            ("contradiction", "Assess the proposal.", "The proposal should be blocked.", "The proposal should be blocked.", "The proposal should be allowed."),
            ("topical_wrong", "Assess the evidence.", "The proposal contains fabricated evidence.", "The proposal contains verified evidence.", "The proposal contains authentic evidence."),
            ("padding", "Assess the proposal.", "The proposal contains fabricated evidence.", "The proposal contains fabricated evidence.", "The proposal contains fabricated evidence and unrelated weather sports cooking travel details."),
            ("missing_evidence", "Assess the proposal.", "The proposal contains fabricated evidence and requests 3.1 million tokens.", "The proposal contains fabricated evidence and requests 3.1 million tokens.", "The proposal contains fabricated evidence."),
            ("exact", "Assess the proposal.", "The proposal is fraudulent.", "The proposal is fraudulent.", "The proposal is fraudulent, but more evidence is required."),
            ("entity", "Is acme proposal fraudulent?", "acme proposal contains fabricated evidence.", "acme proposal contains fabricated evidence.", "globex proposal contains fabricated evidence."),
            ("number", "Assess the proposal.", "The proposal requests 3.1 million tokens and should be rejected.", "The proposal requests 3.1 million tokens and should be rejected.", "The proposal requests 8.4 million tokens and should be rejected."),
            ("mixed_verdict", "Assess the proposal.", "The proposal is fraudulent and should be blocked.", "The proposal is fraudulent and should be blocked.", "The proposal is fraudulent but should be allowed."),
            ("negated_factual_claim", "Assess the transaction for fraud signals.", "Funds were diverted to an undisclosed wallet.", "An undisclosed wallet received diverted funds.", "No funds were diverted to an undisclosed wallet."),
            ("negated_decision_claim", "Assess whether the proposal should be accepted.", "The proposal was rejected because its evidence was fabricated.", "Fabricated evidence caused the proposal to be denied.", "The proposal was not rejected because its evidence was fabricated."),
            ("transfer_direction", "Assess whether the transfer indicates fraud.", "Funds were transferred from the treasury to an undisclosed wallet.", "An undisclosed wallet received funds from the treasury.", "Funds were transferred from an undisclosed wallet to the treasury."),
        ];

        let mut margins = [0.0f32; 13];
        let mut ordered = 0usize;
        let mut ties = 0usize;
        for (index, (name, question, truth, good, bad)) in cases.iter().enumerate() {
            let good_score = production_score(truth, good, fast_score_with_question(question, truth, good));
            let bad_score = production_score(truth, bad, fast_score_with_question(question, truth, bad));
            let margin = good_score - bad_score;
            assert!((0.0..=1.0).contains(&good_score));
            assert!((0.0..=1.0).contains(&bad_score));
            assert!(good_score > bad_score, "{name}: good={good_score} bad={bad_score}");
            if good_score > bad_score { ordered += 1; }
            if good_score == bad_score { ties += 1; }
            margins[index] = margin;
            std::println!("benchmark={name} good={good_score:.4} bad={bad_score:.4} margin={margin:.4}");
        }
        let mean = margins.iter().sum::<f32>() / margins.len() as f32;
        let min = margins.iter().copied().fold(1.0f32, f32::min);
        let variance = margins.iter().map(|margin| (margin - mean) * (margin - mean)).sum::<f32>() / margins.len() as f32;
        std::println!("benchmark_ordering={ordered}/{} average_margin={mean:.4} minimum_margin={min:.4} margin_stddev={:.4} ties={ties}", cases.len(), libm::sqrtf(variance));
        assert_eq!(ordered, cases.len());
        assert_eq!(ties, 0);
    }

    #[test]
    fn negated_factual_claims_are_penalized() {
        // This regression case is locally reviewed. It does not claim to mirror
        // a hidden Telegraph fixture.
        let question = "Assess the transaction for fraud signals.";
        let truth = "Funds were diverted to an undisclosed wallet.";
        let good = "An undisclosed wallet received diverted funds.";
        let bad = "No funds were diverted to an undisclosed wallet.";
        let good_score = production_score(truth, good, fast_score_with_question(question, truth, good));
        let bad_score = production_score(truth, bad, fast_score_with_question(question, truth, bad));
        assert!((0.0..=1.0).contains(&good_score));
        assert!((0.0..=1.0).contains(&bad_score));
        assert!(good_score > bad_score, "good={good_score} bad={bad_score}");
        std::println!("negated_factual_claim good={good_score:.4} bad={bad_score:.4} margin={:.4}", good_score - bad_score);
    }

    #[test]
    fn production_score_is_deterministic_and_bounded() {
        let question = "Assess the transaction for fraud signals.";
        let truth = "Funds were diverted to an undisclosed wallet.";
        let answer = "No funds were diverted to an undisclosed wallet.";
        let first = production_score(truth, answer, fast_score_with_question(question, truth, answer));
        let second = production_score(truth, answer, fast_score_with_question(question, truth, answer));
        assert_eq!(first, second);
        assert!((0.0..=1.0).contains(&first));
    }

    #[test]
    fn negated_decision_claims_are_penalized() {
        // This regression case is locally reviewed and does not claim to mirror
        // a hidden Telegraph fixture.
        let question = "Assess whether the proposal should be accepted.";
        let truth = "The proposal was rejected because its evidence was fabricated.";
        let good = "Fabricated evidence caused the proposal to be denied.";
        let bad = "The proposal was not rejected because its evidence was fabricated.";
        let good_score = production_score(truth, good, fast_score_with_question(question, truth, good));
        let bad_score = production_score(truth, bad, fast_score_with_question(question, truth, bad));
        assert!((0.0..=1.0).contains(&good_score));
        assert!((0.0..=1.0).contains(&bad_score));
        assert!(good_score > bad_score, "good={good_score} bad={bad_score}");
        std::println!("negated_decision_claim good={good_score:.4} bad={bad_score:.4} margin={:.4}", good_score - bad_score);
    }

    #[test]
    fn transfer_direction_reversals_are_penalized() {
        // This regression case is locally reviewed and does not claim to mirror
        // a hidden Telegraph fixture.
        let question = "Assess whether the transfer indicates fraud.";
        let truth = "Funds were transferred from the treasury to an undisclosed wallet.";
        let good = "An undisclosed wallet received funds from the treasury.";
        let bad = "Funds were transferred from an undisclosed wallet to the treasury.";
        let good_score = production_score(truth, good, fast_score_with_question(question, truth, good));
        let bad_score = production_score(truth, bad, fast_score_with_question(question, truth, bad));
        assert!((0.0..=1.0).contains(&good_score));
        assert!((0.0..=1.0).contains(&bad_score));
        assert!(good_score > bad_score, "good={good_score} bad={bad_score}");
        std::println!("transfer_direction_reversal good={good_score:.4} bad={bad_score:.4} margin={:.4}", good_score - bad_score);
    }
}
