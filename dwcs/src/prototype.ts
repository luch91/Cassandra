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

function isAsciiPunctuation(code: number): boolean {
  return (
    (code >= 33 && code <= 47) ||
    (code >= 58 && code <= 64) ||
    (code >= 91 && code <= 96) ||
    (code >= 123 && code <= 126)
  );
}

function normalizedToken(token: string): string {
  let start = 0;
  let end = token.length;
  while (start < end && isAsciiPunctuation(token.charCodeAt(start))) start++;
  while (end > start && isAsciiPunctuation(token.charCodeAt(end - 1))) end--;
  return token.slice(start, end);
}

function equalToken(a: string, b: string): boolean {
  return normalizedToken(a).toLowerCase() === normalizedToken(b).toLowerCase();
}

function matchesAny(token: string, candidates: string[]): boolean {
  return candidates.some((candidate) => equalToken(token, candidate));
}

function isFraudTerm(token: string): boolean {
  return matchesAny(token, ["fraud", "fraudulent", "scam", "scammed", "scamming"]);
}

function isSafeTerm(token: string): boolean {
  return matchesAny(token, ["safe", "legitimate", "legit", "valid", "authentic"]);
}

function semanticEqual(a: string, b: string): boolean {
  return equalToken(a, b) || (isFraudTerm(a) && isFraudTerm(b)) || (isSafeTerm(a) && isSafeTerm(b));
}

function isOpposite(a: string, b: string): boolean {
  const disclosurePair =
    (matchesAny(a, ["disclose", "disclosed", "disclosure"]) &&
      matchesAny(b, ["conceal", "concealed", "hide", "hidden"])) ||
    (matchesAny(b, ["disclose", "disclosed", "disclosure"]) &&
      matchesAny(a, ["conceal", "concealed", "hide", "hidden"]));
  const approvalPair =
    (matchesAny(a, ["approve", "approved", "approval"]) &&
      matchesAny(b, ["reject", "rejected", "deny", "denied"])) ||
    (matchesAny(b, ["approve", "approved", "approval"]) &&
      matchesAny(a, ["reject", "rejected", "deny", "denied"]));
  return disclosurePair || approvalPair || (isFraudTerm(a) && isSafeTerm(b)) || (isFraudTerm(b) && isSafeTerm(a));
}

function isNegation(token: string): boolean {
  return matchesAny(token, ["no", "not", "never", "without", "cannot", "cant", "false"]);
}

function isNegated(tokens: string[], index: number): boolean {
  return (index > 0 && isNegation(tokens[index - 1])) || (index > 1 && isNegation(tokens[index - 2]));
}

function hasPolarityConflict(answer: string[], truth: string[]): boolean {
  return answer.some(
    (answerToken, answerIndex) =>
      !isNegation(answerToken) &&
      truth.some(
        (truthToken, truthIndex) =>
          semanticEqual(answerToken, truthToken) && isNegated(answer, answerIndex) !== isNegated(truth, truthIndex)
      )
  );
}

function isNumericToken(token: string): boolean {
  const normalized = normalizedToken(token);
  return /\d/.test(normalized) && /^[\d,.$%]+$/.test(normalized);
}

function numericEqual(a: string, b: string): boolean {
  const compactA = normalizedToken(a).replace(/[^\d.]/g, "");
  const compactB = normalizedToken(b).replace(/[^\d.]/g, "");
  return compactA.length > 0 && compactA === compactB;
}

function hasNumericConflict(answer: string[], truth: string[]): boolean {
  const answerNumbers = answer.filter(isNumericToken);
  const truthNumbers = truth.filter(isNumericToken);
  return answerNumbers.length > 0 && truthNumbers.length > 0 && answerNumbers.some((number) => !truthNumbers.some((truthNumber) => numericEqual(number, truthNumber)));
}

function hasLexicalOpposition(answer: string[], truth: string[]): boolean {
  return answer.some((answerToken) => truth.some((truthToken) => isOpposite(answerToken, truthToken)));
}

function wordOverlap(answer: string[], truth: string[]): number {
  if (answer.length === 0) return 0;
  const matched = answer.filter((w) => truth.some((truthWord) => semanticEqual(w, truthWord))).length;
  return matched / answer.length;
}

function stopwordWeightedOverlap(answer: string[], truth: string[]): number {
  if (answer.length === 0) return 0;
  let matchedWeight = 0;
  let totalWeight = 0;
  for (const w of answer) {
    const weight = STOPWORDS.has(normalizedToken(w).toLowerCase()) ? 0.3 : 1.0;
    totalWeight += weight;
    if (truth.some((truthWord) => semanticEqual(w, truthWord))) matchedWeight += weight;
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
        semanticEqual(answer[i], truth[j]) && semanticEqual(answer[i + 1], truth[j + 1])
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
      if (semanticEqual(answer[i - 1], truth[j - 1])) {
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

  let finalScore = combine(metrics);
  if (hasPolarityConflict(answerWords, truthWords) || hasLexicalOpposition(answerWords, truthWords)) {
    finalScore *= 0.3;
  } else if (hasNumericConflict(answerWords, truthWords)) {
    finalScore *= 0.45;
  }

  return {
    wordOverlap: metrics[0],
    stopwordWeightedOverlap: metrics[1],
    bigramJaccard: metrics[2],
    lcsRatio: metrics[3],
    variance: v,
    finalScore,
  };
}

export function scorePair(groundTruth: string, minerAnswer: string): number {
  return scorePairWithBreakdown(groundTruth, minerAnswer).finalScore;
}
