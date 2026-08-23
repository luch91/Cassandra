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
 * Layer 2 (our own addition, still an open decision): an explicit "flag
 * this governance proposal" write to whatever DAO/governance contract the
 * chosen document stream (PROJECT_SPEC.md Section 5.2) actually uses.
 * Telegraph does not provide this, it is not a documentation gap, it's a
 * real scope item that depends on picking a specific governance target.
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
 * Layer 2: NOT IMPLEMENTED. Blocked on choosing a specific governance
 * contract/interface once the document stream (PROJECT_SPEC.md Section
 * 5.2) is finalized. This is a real product decision, not something to
 * guess your way past, different DAOs expose completely different
 * governance contract shapes (Governor Bravo-style, Snapshot + a custom
 * execution module, a bespoke contract, etc).
 */
export async function executeLayer2GovernanceFlag(
  _proposal: GovernanceProposal,
  _decision: TriageDecision
): Promise<Layer2ActionResult> {
  throw new Error(
    "Not implemented: Layer 2 governance-contract flag action depends on " +
    "which specific governance target Sentinel is built against. Decide " +
    "the document stream/target contract first (PROJECT_SPEC.md Section " +
    "5.2), then implement against that contract's real interface, never " +
    "a guessed one."
  );
}
