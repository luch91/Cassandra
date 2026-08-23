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
});
