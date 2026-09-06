/**
 * Sentinel entry point. Wires ingestion, multi-miner assessment,
 * agreement-based triage, and Layer 1 receipt collection.
 */

import { fetchPendingProposals } from "./ingest/governance_source";
import { askMultipleMiners, assertPaidRequestsEnabled } from "./scoring/telegraph_client";
import { computeAgreement, decideTriageAction, MIN_MINER_SAMPLE_SIZE } from "./scoring/multi_miner_agreement";
import { verifyLayer1Receipts } from "./onchain/action";
import { appendLayer1Evidence } from "./onchain/receipt_evidence";
import { loadSentinelConfig, type SentinelConfig } from "./config";
import { appendRequestLedger, readProcessedProposalIds } from "./usage/request_ledger";
import { appendProposalAttempt, readAttemptedProposalIds } from "./usage/proposal_attempts";
import { extractRiskSignal } from "./scoring/risk_signal";

export function isActionableBeforeDeadline(proposal: { votingEndsAt: string }, config: SentinelConfig, now = new Date()): boolean {
  const deadline = new Date(proposal.votingEndsAt).getTime() - config.minimumRemainingVoteMinutes * 60_000;
  return Number.isFinite(deadline) && now.getTime() < deadline;
}

export async function runSentinelCycle(config = loadSentinelConfig()): Promise<void> {
  assertPaidRequestsEnabled();
  const proposals = await fetchPendingProposals();
  const processedProposalIds = await readProcessedProposalIds();
  const attemptedProposalIds = await readAttemptedProposalIds();

  for (const proposal of proposals) {
    if (processedProposalIds.has(proposal.id) || attemptedProposalIds.has(proposal.id)) {
      console.log(`Proposal ${proposal.id} has already been attempted, skipping to prevent duplicate traffic.`);
      continue;
    }
    if (!isActionableBeforeDeadline(proposal, config)) {
      console.log(`Proposal ${proposal.id} is too close to its voting deadline, skipping.`);
      continue;
    }
    const query = `Does this governance proposal show signs of fraud or fabricated evidence? Proposal: ${proposal.title}\n\n${proposal.bodyText}`;

    await appendProposalAttempt(proposal.id, "FRAUD_DETECTION");
    const askResults = await askMultipleMiners("FRAUD_DETECTION", query, MIN_MINER_SAMPLE_SIZE, (result) =>
      appendRequestLedger(proposal.id, "FRAUD_DETECTION", result)
    );

    const agreement = computeAgreement(
      askResults.map((r) => ({ minerId: r.miner_id, answerText: JSON.stringify(r.result) }))
    );

    const signals = askResults.map((result) => extractRiskSignal(result.result));
    const escalationSupported = signals.filter((signal) => signal.supportsEscalation).length >= 2;
    const reviewSupported = signals.filter((signal) => signal.requiresReview).length >= 2;
    const decision = decideTriageAction(agreement, config.escalationThreshold, escalationSupported, reviewSupported);

    // Verify each Layer 1 receipt independently before reporting it as evidence.
    const receipts = await verifyLayer1Receipts(askResults);
    await appendLayer1Evidence(proposal.id, decision, receipts);
    console.log(`Proposal ${proposal.id}: ${decision.action}, ${decision.reason}`, receipts);

    if (decision.action === "escalate_for_review") {
      // Snapshot does not offer a universal governance-contract flag write.
      // Preserve the real payment receipt and hand this case to human review.
      console.log(`Proposal ${proposal.id} requires human review.`);
    }
  }
}
