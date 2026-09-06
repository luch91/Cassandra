import { extractRiskSignal } from "../src/scoring/risk_signal";

describe("extractRiskSignal", () => {
  it("requires explicit structured risk before supporting escalation", () => {
    expect(extractRiskSignal({ risk_tier: "high_risk" })).toEqual({ requiresReview: true, supportsEscalation: true });
    expect(extractRiskSignal({ risk_score: 0.6 })).toEqual({ requiresReview: true, supportsEscalation: true });
    expect(extractRiskSignal({ is_suspicious: true })).toEqual({ requiresReview: true, supportsEscalation: true });
  });

  it("routes evidence gaps to review without treating them as fraud", () => {
    expect(extractRiskSignal({ gate_decision: "RECHECK" })).toEqual({ requiresReview: true, supportsEscalation: false });
    expect(extractRiskSignal({ gate_decision: "ALLOW" })).toEqual({ requiresReview: false, supportsEscalation: false });
  });

  it("never infers risk from free-form text alone", () => {
    expect(extractRiskSignal({ signal: "This sounds fraudulent" })).toEqual({ requiresReview: false, supportsEscalation: false });
  });
});
