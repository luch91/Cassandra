import {
  computeAgreement,
  decideTriageAction,
  DEFAULT_ESCALATION_THRESHOLD,
  MIN_MINER_SAMPLE_SIZE,
  type MinerAnswer,
} from "../src/scoring/multi_miner_agreement";

describe("computeAgreement", () => {
  it("returns full agreement trivially for a single answer (and callers should not trust this)", () => {
    const answers: MinerAnswer[] = [{ minerId: "1", answerText: "this proposal looks legitimate" }];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBe(1);
    expect(result.sampleSize).toBe(1);
  });

  it("scores high agreement when miners give similar answers", () => {
    const answers: MinerAnswer[] = [
      { minerId: "1", answerText: "this proposal shows signs of fraud and fabricated evidence" },
      { minerId: "2", answerText: "signs of fraud and fabricated evidence are present in this proposal" },
      { minerId: "3", answerText: "fraud and fabricated evidence detected in this proposal" },
    ];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBeGreaterThan(0.5);
  });

  it("scores low agreement when miners disagree", () => {
    const answers: MinerAnswer[] = [
      { minerId: "1", answerText: "this proposal is completely legitimate and well documented" },
      { minerId: "2", answerText: "fraud detected fabricated evidence manipulation" },
      { minerId: "3", answerText: "unable to determine anything from the provided text" },
    ];
    const result = computeAgreement(answers);
    expect(result.agreementScore).toBeLessThan(0.5);
  });
});

describe("decideTriageAction", () => {
  it("flags for review when sample size is below the minimum", () => {
    const decision = decideTriageAction({ agreementScore: 0.99, sampleSize: 1, representativeAnswer: "x" });
    expect(decision.action).toBe("flag_for_review");
  });

  it("flags for review when agreement is low even with enough samples", () => {
    const decision = decideTriageAction({
      agreementScore: 0.2,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("flag_for_review");
  });

  it("escalates on-chain when agreement clears the threshold with enough samples", () => {
    const decision = decideTriageAction({
      agreementScore: DEFAULT_ESCALATION_THRESHOLD + 0.05,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("escalate_onchain");
  });

  it("takes no action when agreement is moderate but below threshold", () => {
    const decision = decideTriageAction({
      agreementScore: 0.6,
      sampleSize: MIN_MINER_SAMPLE_SIZE,
      representativeAnswer: "x",
    });
    expect(decision.action).toBe("no_action");
  });
});
