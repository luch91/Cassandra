import { verifyLayer1Receipts } from "../src/onchain/action";
import type { AskResult } from "../src/scoring/telegraph_client";

function verifiedFixture(signalHash: string) {
  return {
    signal_hash: signalHash,
    result: { status: "success" as const },
    verification: {
      verified: true as const,
      algorithm: "keccak256",
      commitment: "payload",
    },
  };
}

const askResultFixtures: AskResult[] = [
  {
    miner_id: "miner-1",
    miner_name: "Fixture Miner One",
    result: { verdict: "review" },
    cost_usd: 0.01,
    duration_ms: 100,
    signal_hash: "signal-1",
  },
  {
    miner_id: "miner-2",
    miner_name: "Fixture Miner Two",
    result: { verdict: "review" },
    cost_usd: 0.01,
    duration_ms: 120,
    signal_hash: "signal-2",
  },
];

describe("verifyLayer1Receipts", () => {
  it("verifies and preserves every paid request receipt", async () => {
    const verifier = jest.fn(async (signalHash: string) => verifiedFixture(signalHash));

    const receipts = await verifyLayer1Receipts(askResultFixtures, verifier);

    expect(verifier).toHaveBeenCalledTimes(2);
    expect(verifier).toHaveBeenNthCalledWith(1, "signal-1");
    expect(verifier).toHaveBeenNthCalledWith(2, "signal-2");
    expect(receipts).toHaveLength(2);
    expect(receipts[0]).toMatchObject({
      signalHash: "signal-1",
      minerId: "miner-1",
      verification: verifiedFixture("signal-1"),
    });
    expect(receipts[0].verifiedAt).not.toBe("");
  });

  it("does not produce a verified receipt when verification fails", async () => {
    const verifier = jest.fn(async () => {
      throw new Error("verification unavailable");
    });

    await expect(verifyLayer1Receipts(askResultFixtures, verifier)).rejects.toThrow(
      "verification unavailable"
    );
  });

  it("rejects missing signal hashes before making verification requests", async () => {
    const verifier = jest.fn(async () => verifiedFixture("signal-1"));
    const invalidFixtures = [
      askResultFixtures[0],
      { ...askResultFixtures[1], signal_hash: " " },
    ];

    await expect(verifyLayer1Receipts(invalidFixtures, verifier)).rejects.toThrow(
      "signal_hash is empty"
    );
    expect(verifier).not.toHaveBeenCalled();
  });

  it("rejects an empty ask result set", async () => {
    const verifier = jest.fn(async () => verifiedFixture("signal-1"));

    await expect(verifyLayer1Receipts([], verifier)).rejects.toThrow(
      "without ask results"
    );
    expect(verifier).not.toHaveBeenCalled();
  });
});
