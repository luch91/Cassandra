/** Conservative interpretation of heterogeneous miner results for triage. */
export interface RiskSignal {
  requiresReview: boolean;
  supportsEscalation: boolean;
}

const HIGH_RISK_TIERS = new Set(["ELEVATED_RISK", "HIGH_RISK", "ELEVATED", "HIGH", "SEVERE"]);
const REVIEW_GATE_DECISIONS = new Set(["RECHECK", "BLOCK"]);

export function extractRiskSignal(result: unknown): RiskSignal {
  if (!result || typeof result !== "object") {
    return { requiresReview: false, supportsEscalation: false };
  }
  const record = result as Record<string, unknown>;
  const riskScore = typeof record.risk_score === "number" ? record.risk_score : undefined;
  const riskTier = typeof record.risk_tier === "string" ? record.risk_tier.toUpperCase() : "";
  const riskLevel = typeof record.risk_level === "string" ? record.risk_level.toUpperCase() : "";
  const isSuspicious = record.is_suspicious === true;
  const supportsEscalation = isSuspicious || (riskScore !== undefined && riskScore >= 0.6) || HIGH_RISK_TIERS.has(riskTier) || HIGH_RISK_TIERS.has(riskLevel);
  const gateDecision = typeof record.gate_decision === "string" ? record.gate_decision.toUpperCase() : "";
  return {
    requiresReview: supportsEscalation || REVIEW_GATE_DECISIONS.has(gateDecision),
    supportsEscalation,
  };
}
