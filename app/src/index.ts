/**
 * Sentinel entry point. Wires ingestion, multi-miner assessment,
 * agreement-based triage, and Layer 1 receipt collection.
 */

import { fetchPendingProposals } from "./ingest/governance_source";
import { askMultipleMiners } from "./scoring/telegraph_client";
import { computeAgreement, decideTriageAction, MIN_MINER_SAMPLE_SIZE } from "./scoring/multi_miner_agreement";
import { collectLayer1Receipts } from "./onchain/action";

export async function runSentinelCycle(): Promise<void> {
  const proposals = await fetchPendingProposals();

  for (const proposal of proposals) {
    const query = `Does this governance proposal show signs of fraud or fabricated evidence? Proposal: ${proposal.title}\n\n${proposal.bodyText}`;

    const askResults = await askMultipleMiners("FRAUD_DETECTION", query, MIN_MINER_SAMPLE_SIZE);

    const agreement = computeAgreement(
      askResults.map((r) => ({ minerId: r.miner_id, answerText: JSON.stringify(r.result) }))
    );

    const decision = decideTriageAction(agreement);

    // Layer 1 always happens, it's just collecting what already occurred.
    const receipt = collectLayer1Receipts(askResults);
    console.log(`Proposal ${proposal.id}: ${decision.action}, ${decision.reason}`, receipt);

    if (decision.action === "escalate_onchain") {
      // Snapshot does not offer a universal governance-contract flag write.
      // Preserve the real payment receipt and hand this case to human review.
      console.log(`Proposal ${proposal.id} requires human review.`);
    }
  }
}
