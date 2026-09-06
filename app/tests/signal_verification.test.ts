import {
  verifySignal,
  type FetchLike,
} from "../src/scoring/telegraph_client";

const signalHash = "0xf9ae57927c19dd29a14cb368e4d2180fdf72cd1fe57ebb959427c1d9ad940591";

function verificationFixture(overrides: Record<string, unknown> = {}) {
  return {
    signal_hash: signalHash,
    result: { status: "success" },
    verification: {
      verified: true,
      algorithm: "keccak256",
      commitment: "payload",
    },
    ...overrides,
  };
}

function response(payload: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => payload,
  } as Response;
}

describe("verifySignal", () => {
  it("accepts only an exact successful cryptographically verified receipt", async () => {
    const fetcher = jest.fn(async () => response(verificationFixture())) as unknown as FetchLike;

    await expect(verifySignal(signalHash, fetcher)).resolves.toMatchObject(verificationFixture());
    expect(fetcher).toHaveBeenCalledWith(
      expect.stringContaining(`/engine/v1/signal/${encodeURIComponent(signalHash)}`)
    );
  });

  it.each([
    ["a different receipt hash", verificationFixture({ signal_hash: "0xother" }), "different signal hash"],
    ["an unsuccessful result", verificationFixture({ result: { status: "failed" } }), "successful execution"],
    ["an unverified proof", verificationFixture({ verification: { verified: false } }), "cryptographically verify"],
  ])("rejects %s", async (_label, payload, message) => {
    const fetcher = jest.fn(async () => response(payload)) as unknown as FetchLike;

    await expect(verifySignal(signalHash, fetcher)).rejects.toThrow(message);
  });

  it("rejects a missing receipt without a network call", async () => {
    const fetcher = jest.fn(async () => response(verificationFixture())) as unknown as FetchLike;

    await expect(verifySignal(" ", fetcher)).rejects.toThrow("empty signal hash");
    expect(fetcher).not.toHaveBeenCalled();
  });
});
