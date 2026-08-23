/**
 * Governance proposal ingestion for Sentinel.
 *
 * Sentinel monitors the Balancer Snapshot space through Snapshot's public
 * GraphQL API. This is a real public document source, not fixture data.
 */

export interface GovernanceProposal {
  id: string;
  source: string;
  title: string;
  bodyText: string;
  linkedEvidenceUrls: string[];
  submittedAt: string; // ISO 8601
}

export const SNAPSHOT_HUB_URL = "https://hub.snapshot.org/graphql";
export const SNAPSHOT_SPACE = "balancer.eth";

interface SnapshotProposal {
  id: string;
  title: string;
  body: string;
  created: number;
  space: {
    id: string;
    name: string;
  };
}

interface SnapshotResponse {
  data?: {
    proposals?: SnapshotProposal[];
  };
  errors?: Array<{
    message: string;
  }>;
}

const PENDING_PROPOSALS_QUERY = `
  query PendingProposals($space: String!) {
    proposals(
      first: 20
      where: { space_in: [$space], state: "active" }
      orderBy: "created"
      orderDirection: desc
    ) {
      id
      title
      body
      created
      space {
        id
        name
      }
    }
  }
`;

function extractEvidenceUrls(body: string): string[] {
  return [...new Set(body.match(/https?:\/\/[^\s)<>'"`]+/g) ?? [])];
}

/**
 * Fetches currently active proposals from the selected public Snapshot space.
 * Snapshot's active state is the actionable proposal set for Sentinel.
 */
export async function fetchPendingProposals(): Promise<GovernanceProposal[]> {
  const response = await fetch(SNAPSHOT_HUB_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      query: PENDING_PROPOSALS_QUERY,
      variables: { space: SNAPSHOT_SPACE },
    }),
  });

  if (!response.ok) {
    throw new Error(`Snapshot proposal request failed: HTTP ${response.status}`);
  }

  const payload = (await response.json()) as SnapshotResponse;
  if (payload.errors?.length) {
    throw new Error(`Snapshot proposal query failed: ${payload.errors.map((error) => error.message).join("; ")}`);
  }

  return (payload.data?.proposals ?? []).map((proposal) => ({
    id: proposal.id,
    source: `snapshot:${proposal.space.id}`,
    title: proposal.title,
    bodyText: proposal.body,
    linkedEvidenceUrls: extractEvidenceUrls(proposal.body),
    submittedAt: new Date(proposal.created * 1_000).toISOString(),
  }));
}
