/**
 * Governance proposal ingestion for Sentinel.
 * See PROJECT_SPEC.md Section 5.2.
 *
 * TODO: pick a concrete public governance source (a specific DAO forum,
 * Snapshot space, or on-chain governance contract) and implement a real
 * fetcher here. Not blocked by open questions, this just needs a decision.
 * Record that decision as a new row in PROJECT_SPEC.md Section 8 once made,
 * it's a real project decision, not an implementation detail to bury silently.
 */

export interface GovernanceProposal {
  id: string;
  source: string;
  title: string;
  bodyText: string;
  linkedEvidenceUrls: string[];
  submittedAt: string; // ISO 8601
}

export async function fetchPendingProposals(_source: string): Promise<GovernanceProposal[]> {
  throw new Error(
    "Not implemented: pick a concrete governance source and implement a real " +
    "fetcher. See file header comment."
  );
}
