import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { appendProposalAttempt, readAttemptedProposalIds } from "../src/usage/proposal_attempts";

describe("Sentinel proposal attempt ledger", () => {
  it("persists proposals before inference for no-repeat scheduling", async () => {
    const directory = await mkdtemp(join(tmpdir(), "sentinel-attempts-"));
    const filePath = join(directory, "attempts", "proposals.jsonl");

    await appendProposalAttempt("proposal-1", "FRAUD_DETECTION", filePath);

    expect(await readAttemptedProposalIds(filePath)).toEqual(new Set(["proposal-1"]));
  });

  it("rejects an empty proposal ID", async () => {
    await expect(appendProposalAttempt("", "FRAUD_DETECTION", join(tmpdir(), "attempts.jsonl"))).rejects.toThrow("proposal ID");
  });
});
