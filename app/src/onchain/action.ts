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
import {
  verifySignal,
  type AskResult,
  type TelegraphSignalVerification,
} from "../scoring/telegraph_client";

export interface Layer1Receipt {
  signalHash: string;
  verifiedAt: string;
  minerId: string;
  verification: unknown;
}

/**
 * Layer 1: independently verifies every receipt Sentinel received from its
 * paid requests. A timestamp is only attached after Telegraph's verification
 * endpoint returns successfully.
 */
export async function verifyLayer1Receipts(
  askResults: AskResult[],
  verifier: (signalHash: string) => Promise<TelegraphSignalVerification> = verifySignal
): Promise<Layer1Receipt[]> {
  if (askResults.length === 0) {
    throw new Error("Cannot verify Layer 1 receipts without ask results.");
  }

  for (const result of askResults) {
    if (!result.signal_hash?.trim()) {
      throw new Error(`Cannot verify Layer 1 receipt for miner ${result.miner_id}: signal_hash is empty.`);
    }
  }

  return Promise.all(
    askResults.map(async (result) => {
      const signalHash = result.signal_hash!;
      const verification = await verifier(signalHash);
      return {
        signalHash,
        verifiedAt: new Date().toISOString(),
        minerId: result.miner_id,
        verification,
      };
    })
  );
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
