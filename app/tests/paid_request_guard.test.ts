import { assertPaidRequestsEnabled } from "../src/scoring/telegraph_client";
import { runSentinelCycle } from "../src";

describe("Sentinel paid request guard", () => {
  it("requires a separate explicit paid-request opt-in", () => {
    expect(() => assertPaidRequestsEnabled({})).toThrow("Paid Telegraph requests are disabled");
    expect(() => assertPaidRequestsEnabled({ SENTINEL_ALLOW_PAID_REQUESTS: "false" })).toThrow("Paid Telegraph requests are disabled");
    expect(() => assertPaidRequestsEnabled({ SENTINEL_ALLOW_PAID_REQUESTS: "true" })).not.toThrow();
  });

  it("stops a cycle before any network access when paid requests are disabled", async () => {
    const original = process.env.SENTINEL_ALLOW_PAID_REQUESTS;
    delete process.env.SENTINEL_ALLOW_PAID_REQUESTS;
    const fetchSpy = jest.spyOn(global, "fetch");

    await expect(runSentinelCycle()).rejects.toThrow("Paid Telegraph requests are disabled");
    expect(fetchSpy).not.toHaveBeenCalled();

    if (original === undefined) delete process.env.SENTINEL_ALLOW_PAID_REQUESTS;
    else process.env.SENTINEL_ALLOW_PAID_REQUESTS = original;
    fetchSpy.mockRestore();
  });
});
