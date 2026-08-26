/**
 * Sentinel's on-chain action, corrected Aug 22 per decision D10.
 *
 * Two distinct layers, do not conflate them:
 *
 * Layer 1 (confirmed, automatic): every paid x402 request Sentinel makes
 * via telegraph_client.ts already produces an on-chain-settled payment and
 * a signal_hash receipt, independently verifiable and visible on
 * explorer.telegraphprotocol.com. This alone satisfies "must use Telegraph
 * miners" with a real on-chain artifact. No extra code needed beyond what
 * telegraph_client.ts already does.
 *
 * Layer 2 (our own addition) is intentionally not implemented for the
 * selected Snapshot document stream. Snapshot spaces do not expose a common
 * governance-contract flag write. Telegraph does not provide one either.
 * Do not implement this against a guessed contract interface.
 */

import type { TriageDecision } from "../scoring/multi_miner_agreement";
import type { GovernanceProposal } from "../ingest/governance_source";
import type { AskResult } from "../scoring/telegraph_client";

export interface Layer1Receipt {
  signalHash: string;
  verifiedAt: string;
  minerIds: string[];
}

/**
 * Layer 1: just collects the receipts Sentinel already has from its paid
 * requests. This is real and buildable now, it doesn't need anything new.
 */
export function collectLayer1Receipts(askResults: AskResult[]): Layer1Receipt {
  return {
    signalHash: askResults[0]?.signal_hash ?? "",
    verifiedAt: new Date().toISOString(),
    minerIds: askResults.map((r) => r.miner_id),
  };
}

export interface Layer2ActionResult {
  txHash: string;
  action: string;
  confirmedAt: string;
}

/**
 * Layer 2: NOT IMPLEMENTED. The selected Snapshot source has no universal
 * contract interface for flags. A future Layer 2 requires an explicit new
 * target and a verified interface.
 */
export async function executeLayer2GovernanceFlag(
  _proposal: GovernanceProposal,
  _decision: TriageDecision
): Promise<Layer2ActionResult> {
  throw new Error(
    "Not implemented: the selected Snapshot source does not expose a " +
    "universal governance-contract flag action. A future integration needs " +
    "an explicit target and verified contract interface."
  );
}
