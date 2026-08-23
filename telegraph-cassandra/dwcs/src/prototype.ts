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
