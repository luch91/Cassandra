import { isActionableBeforeDeadline } from "../src";
import type { SentinelConfig } from "../src/config";

const config: SentinelConfig = { escalationThreshold: 0.85, minimumRemainingVoteMinutes: 60, pollIntervalMs: 60_000 };

describe("Sentinel proposal deadline", () => {
  it("accepts a proposal with sufficient time remaining", () => {
    expect(isActionableBeforeDeadline({ votingEndsAt: "2026-09-02T14:00:00.000Z" }, config, new Date("2026-09-02T12:00:00.000Z"))).toBe(true);
  });

  it("skips proposals inside the review buffer or with invalid deadlines", () => {
    expect(isActionableBeforeDeadline({ votingEndsAt: "2026-09-02T12:30:00.000Z" }, config, new Date("2026-09-02T12:00:00.000Z"))).toBe(false);
    expect(isActionableBeforeDeadline({ votingEndsAt: "invalid" }, config, new Date())).toBe(false);
  });
});
