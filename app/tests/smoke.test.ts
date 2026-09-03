import { assertPaidSmokeOptIn } from "../src/smoke";

describe("one-shot paid smoke safety gate", () => {
  it("requires the environment opt-in and explicit command confirmation", () => {
    expect(() => assertPaidSmokeOptIn({}, ["--confirm-paid-smoke"])).toThrow("SENTINEL_ALLOW_PAID_REQUESTS=true");
    expect(() => assertPaidSmokeOptIn({ SENTINEL_ALLOW_PAID_REQUESTS: "true" }, [])).toThrow("--confirm-paid-smoke");
  });

  it("accepts both opt-ins", () => {
    expect(() => assertPaidSmokeOptIn({ SENTINEL_ALLOW_PAID_REQUESTS: "true" }, ["--confirm-paid-smoke"])).not.toThrow();
  });
});
