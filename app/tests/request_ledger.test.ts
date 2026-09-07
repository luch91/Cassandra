import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  appendRequestLedger,
  assertCanStartPaidRequest,
  getUsageMetrics,
  readProcessedProposalIds,
  UsageBudgetReachedError,
} from "../src/usage/request_ledger";

const result = {
  miner_id: "miner-1",
  miner_name: "Fixture Miner",
  result: { verdict: "review" },
  cost_usd: 0.01,
  duration_ms: 42,
  signal_hash: "signal-1",
};

describe("Sentinel request ledger", () => {
  it("records attributable completed requests and supports deduplication", async () => {
    const directory = await mkdtemp(join(tmpdir(), "sentinel-ledger-"));
    const filePath = join(directory, "usage", "requests.jsonl");

    const record = await appendRequestLedger("proposal-1", "FRAUD_DETECTION", result, filePath);

    expect(record).toMatchObject({ proposalId: "proposal-1", intent: "FRAUD_DETECTION", minerId: "miner-1" });
    expect(await readProcessedProposalIds(filePath)).toEqual(new Set(["proposal-1"]));
    expect(JSON.parse((await readFile(filePath, "utf8")).trim())).toEqual(record);
    await appendRequestLedger("proposal-2", "FRAUD_DETECTION", { ...result, miner_id: "miner-2", cost_usd: 0.02 }, filePath);
    await expect(getUsageMetrics(filePath, 2)).resolves.toEqual({
      completedRequests: 2,
      uniqueProposals: 2,
      totalCostUsd: 0.03,
      targetRequests: 2,
      targetReached: true,
    });
  });

  it("treats a missing ledger as no prior usage", async () => {
    expect(await readProcessedProposalIds(join(tmpdir(), "missing-sentinel-ledger.jsonl"))).toEqual(new Set());
  });

  it("rejects an invalid usage target", async () => {
    await expect(getUsageMetrics(undefined, 0)).rejects.toThrow("positive integer");
  });

  it("fails closed at the request and budget limits", () => {
    const budget = { maxCompletedRequests: 100, maxBudgetUsd: 1, maxRequestCostUsd: 0.01 };
    expect(() =>
      assertCanStartPaidRequest(
        { completedRequests: 100, uniqueProposals: 100, totalCostUsd: 1, targetRequests: 100, targetReached: true },
        budget,
      ),
    ).toThrow(UsageBudgetReachedError);
    expect(() =>
      assertCanStartPaidRequest(
        { completedRequests: 99, uniqueProposals: 99, totalCostUsd: 0.995, targetRequests: 100, targetReached: false },
        budget,
      ),
    ).toThrow("budget would be exceeded");
    expect(() =>
      assertCanStartPaidRequest(
        { completedRequests: 99, uniqueProposals: 99, totalCostUsd: 0.98, targetRequests: 100, targetReached: false },
        budget,
      ),
    ).not.toThrow();
  });
});
