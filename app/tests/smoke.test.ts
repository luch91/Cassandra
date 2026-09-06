import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runSentinelSmoke, selectProposalSmokeMiner } from "../src/smoke";

const route = (id: string, slug: string, path: string, description = "FRAUD_DETECTION") => ({
  id, slug, name: slug, endpoint: { method: "POST" as const, path, description },
});

describe("selectProposalSmokeMiner", () => {
  it("prefers the reviewed proposal-semantic SarzOps route regardless of registry order", () => {
    const selected = selectProposalSmokeMiner([
      route("10002", "degenlens-onchain", "/anomaly/check"),
      route("9002", "txlens", "/fraud-query"),
      route("91001", "sarzops-transaction-risk", "/fraud"),
    ], "a proposal query");
    expect(selected.id).toBe("91001");
  });

  it("uses TxLens only for queries at or below its declared limit", () => {
    const miner = route("9002", "txlens", "/fraud-query");
    expect(selectProposalSmokeMiner([miner], "x".repeat(1000)).id).toBe("9002");
    expect(() => selectProposalSmokeMiner([miner], "x".repeat(1001))).toThrow("query length");
  });

  it("refuses unrelated or address-only routes", () => {
    expect(() => selectProposalSmokeMiner([route("302", "chainsight-oracle", "/fraud", "FRAUD_DETECTION")], "query"))
      .toThrow("proposal-semantic");
  });
});

describe("runSentinelSmoke", () => {
  it("makes one injected request and writes redacted verified evidence", async () => {
    const directory = await mkdtemp(join(tmpdir(), "sentinel-smoke-"));
    const ask = jest.fn(async () => ({
      miner_id: "302", miner_name: "Fixture Miner", result: { answer: "fixture only" },
      cost_usd: 0.01, duration_ms: 20_501, signal_hash: "signal-1",
    }));
    const verify = jest.fn(async () => [{
      signalHash: "signal-1", verifiedAt: "2026-09-04T00:00:00.000Z", minerId: "302",
      verification: {
        signal_hash: "signal-1", result: { status: "success" as const },
        verification: { verified: true as const, algorithm: "keccak256", commitment: "payload" },
      },
    }]);
    const evidencePath = join(directory, "evidence.json");
    const evidence = await runSentinelSmoke("fixture query", { SENTINEL_ALLOW_PAID_REQUESTS: "true" }, {
      discover: async () => [{ id: "302", slug: "chainsight-oracle", name: "ChainSight", endpoint: { method: "GET", path: "/fraud" } }],
      ask, verify,
    }, evidencePath);
    expect(ask).toHaveBeenCalledTimes(1);
    expect(verify).toHaveBeenCalledTimes(1);
    expect(evidence).toMatchObject({ minerId: "302", signalHash: "signal-1", verification: { algorithm: "keccak256" } });
    const saved = await readFile(evidencePath, "utf8");
    expect(saved).not.toContain("fixture only");
    expect(JSON.parse(saved)).toEqual(evidence);
  });

  it("requires explicit paid-request authorization before discovery", async () => {
    const discover = jest.fn(async () => []);
    await expect(runSentinelSmoke("query", {}, { discover, ask: jest.fn(), verify: jest.fn() }, "evidence.json"))
      .rejects.toThrow("Paid Telegraph requests are disabled");
    expect(discover).not.toHaveBeenCalled();
  });
});
