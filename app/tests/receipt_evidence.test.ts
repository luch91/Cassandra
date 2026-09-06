import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { appendLayer1Evidence } from "../src/onchain/receipt_evidence";
import type { Layer1Receipt } from "../src/onchain/action";

const receipt: Layer1Receipt = {
  signalHash: "signal-1",
  verifiedAt: "2026-08-31T00:00:00.000Z",
  minerId: "miner-1",
  verification: { signal_hash: "signal-1", status: "settled" },
};

describe("appendLayer1Evidence", () => {
  it("appends a complete JSONL evidence record", async () => {
    const directory = await mkdtemp(join(tmpdir(), "sentinel-evidence-"));
    const filePath = join(directory, "nested", "receipts.jsonl");

    const record = await appendLayer1Evidence(
      "proposal-1",
      { action: "escalate_for_review" },
      [receipt],
      filePath
    );

    expect(record).toMatchObject({ proposalId: "proposal-1", action: "escalate_for_review", receipts: [receipt] });
    expect(JSON.parse((await readFile(filePath, "utf8")).trim())).toEqual(record);
  });

  it("rejects incomplete evidence before creating a file", async () => {
    const directory = await mkdtemp(join(tmpdir(), "sentinel-evidence-"));
    const filePath = join(directory, "receipts.jsonl");

    await expect(
      appendLayer1Evidence("proposal-1", { action: "no_action" }, [{ ...receipt, signalHash: "" }], filePath)
    ).rejects.toThrow("incomplete receipt");
  });
});
