import { loadSentinelConfig } from "../src/config";

describe("loadSentinelConfig", () => {
  it("uses documented conservative defaults", () => {
    expect(loadSentinelConfig({})).toEqual({
      escalationThreshold: 0.85,
      minimumRemainingVoteMinutes: 60,
      pollIntervalMs: 900_000,
    });
  });

  it("accepts safe overrides", () => {
    expect(loadSentinelConfig({
      SENTINEL_ESCALATION_THRESHOLD: "0.9",
      SENTINEL_MIN_REMAINING_VOTE_MINUTES: "120",
      SENTINEL_POLL_INTERVAL_MS: "60000",
    })).toEqual({ escalationThreshold: 0.9, minimumRemainingVoteMinutes: 120, pollIntervalMs: 60_000 });
  });

  it("rejects unsafe polling and thresholds", () => {
    expect(() => loadSentinelConfig({ SENTINEL_POLL_INTERVAL_MS: "1" })).toThrow("SENTINEL_POLL_INTERVAL_MS");
    expect(() => loadSentinelConfig({ SENTINEL_ESCALATION_THRESHOLD: "0.2" })).toThrow("SENTINEL_ESCALATION_THRESHOLD");
  });
});
