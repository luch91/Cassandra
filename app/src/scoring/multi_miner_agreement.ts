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

