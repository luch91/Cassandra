import { scorePair, scorePairWithBreakdown } from "../src/prototype";

describe("scorePair (TS prototype, mirrors dwcs/rust-module/src/lib.rs)", () => {
  it("scores an empty answer exactly 0", () => {
    expect(scorePair("Paris is the capital of France.", "")).toBe(0);
    expect(scorePair("Paris is the capital of France.", "   ")).toBe(0);
  });

  it("scores an exact match exactly 1", () => {
    expect(scorePair("Paris is the capital of France.", "Paris is the capital of France.")).toBe(1);
  });

  it("scores a correct answer above an unrelated one", () => {
    const gt = "Paris is the capital of France.";
    const good = scorePair(gt, "The capital of France is Paris.");
    const bad = scorePair(gt, "Bananas are yellow and grow on trees.");
    expect(good).toBeGreaterThan(bad);
  });

  it("recognizes a reworded correct answer better than an unrelated one", () => {
    const gt = "The mitochondria is the powerhouse of the cell.";
    const reworded = scorePair(gt, "Mitochondria act as the cell's powerhouse.");
    const unrelated = scorePair(gt, "Stock prices fell sharply on Tuesday.");
    expect(reworded).toBeGreaterThan(unrelated);
  });

  it("does not let keyword stuffing beat an actually correct answer", () => {
    const gt = "The Eiffel Tower is located in Paris, France.";
    const stuffed = scorePair(gt, "Paris France Eiffel Tower Paris France located located.");
    const correct = scorePair(gt, "The Eiffel Tower stands in Paris, France.");
    expect(correct).toBeGreaterThanOrEqual(stuffed);
  });

  it("exposes a full metric breakdown for tuning", () => {
    const breakdown = scorePairWithBreakdown(
      "The mitochondria is the powerhouse of the cell.",
      "Mitochondria act as the cell's powerhouse."
    );
    expect(breakdown).toHaveProperty("wordOverlap");
    expect(breakdown).toHaveProperty("bigramJaccard");
    expect(breakdown).toHaveProperty("lcsRatio");
    expect(breakdown).toHaveProperty("variance");
    expect(breakdown.finalScore).toBeGreaterThan(0);
  });

  it("orders local fraud-relevant correctness fixtures", () => {
    // Local correctness fixtures only. They are not Miner data and are never
    // sent to Telegraph.
    const cases: Array<[string, string, string]> = [
      ["The proposal transfers 5,000 USDC.", "The proposal transfers 5000 USDC.", "The proposal transfers 500 USDC."],
      ["The proposal is fraudulent.", "This is a scam.", "This is legitimate."],
      ["The proposal is legitimate.", "The proposal is safe.", "The proposal is a scam."],
      ["The proposal is fraudulent.", "The proposal is fraudulent.", "The proposal is not fraudulent."],
      ["The proposal is not fraudulent.", "The proposal is not fraudulent.", "The proposal is fraudulent."],
      ["The audit must finish before deployment.", "Deployment requires the audit to finish first.", "Deployment does not require an audit."],
      ["Voting closes on 2026-09-01.", "The vote ends on 2026-09-01.", "The vote ends on 2026-09-10."],
      ["The quorum is 100000 tokens.", "A 100000 token quorum is required.", "A 10000 token quorum is required."],
      ["The treasury sends 250 USDC to the contributor.", "The contributor receives 250 USDC from the treasury.", "The treasury sends 2500 USDC to the contributor."],
      ["The claim has no supporting evidence.", "There is no evidence supporting the claim.", "The claim has supporting evidence."],
      ["The bridge has not completed an audit.", "The bridge has not completed its audit.", "The bridge has completed its audit."],
      ["The contract upgrade is approved.", "The upgrade is approved.", "The upgrade is not approved."],
      ["The funds remain in the treasury.", "Treasury funds remain in place.", "The funds are transferred from the treasury."],
      ["The proposal author disclosed the conflict.", "The conflict was disclosed by the author.", "The author concealed the conflict."],
      ["The payment recipient is verified.", "The recipient is verified.", "The recipient is not verified."],
    ];

    for (const [truth, good, bad] of cases) {
      expect(scorePair(truth, good)).toBeGreaterThan(scorePair(truth, bad));
    }
  });
});
