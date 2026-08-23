import { fetchPendingProposals, SNAPSHOT_HUB_URL, SNAPSHOT_SPACE } from "../src/ingest/governance_source";

describe("Snapshot governance source", () => {
  afterEach(() => {
    jest.restoreAllMocks();
  });

  it("maps a Snapshot response fixture into a governance proposal", async () => {
    // Fixture only. Production code always calls Snapshot's public endpoint.
    const fetchFixture = jest.spyOn(global, "fetch").mockResolvedValue({
      ok: true,
      json: async () => ({
        data: {
          proposals: [
            {
              id: "proposal-1",
              title: "Fund security review",
              body: "Review details: https://example.org/evidence and https://example.org/evidence",
              created: 1_787_304_511,
              space: { id: "balancer.eth", name: "Balancer" },
            },
          ],
        },
      }),
    } as Response);

    await expect(fetchPendingProposals()).resolves.toEqual([
      {
        id: "proposal-1",
        source: "snapshot:balancer.eth",
        title: "Fund security review",
        bodyText: "Review details: https://example.org/evidence and https://example.org/evidence",
        linkedEvidenceUrls: ["https://example.org/evidence"],
        submittedAt: "2026-08-21T09:28:31.000Z",
      },
    ]);

    expect(fetchFixture).toHaveBeenCalledWith(
      SNAPSHOT_HUB_URL,
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining(SNAPSHOT_SPACE),
      })
    );
  });

  it("fails clearly when Snapshot returns a GraphQL error", async () => {
    // Fixture only. This verifies error handling without creating network traffic.
    jest.spyOn(global, "fetch").mockResolvedValue({
      ok: true,
      json: async () => ({ errors: [{ message: "invalid query" }] }),
    } as Response);

    await expect(fetchPendingProposals()).rejects.toThrow("Snapshot proposal query failed: invalid query");
  });
});
