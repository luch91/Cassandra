/**
 * Sentinel entry point, corrected Aug 22. Wires ingestion -> multi-miner
 * ask -> agreement-based triage -> Layer 1 receipt collection -> Layer 2
 * governance flag (once that decision is made).
 */

import { fetchPendingProposals } from "./ingest/governance_source";
import { askMultipleMiners } from "./scoring/telegraph_client";
import { computeAgreement, decideTriageAction, MIN_MINER_SAMPLE_SIZE } from "./scoring/multi_miner_agreement";
import { collectLayer1Receipts, executeLayer2GovernanceFlag } from "./onchain/action";

export async function runSentinelCycle(source: string): Promise<void> {
  const proposals = await fetchPendingProposals(source);

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
      // Layer 2 is still an open decision, see onchain/action.ts. This
      // call will throw until a governance target is chosen and
      // implemented against its real contract interface.
      await executeLayer2GovernanceFlag(proposal, decision);
    }
  }
}
