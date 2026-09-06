import { mkdir, appendFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { TriageDecision } from "../scoring/multi_miner_agreement";
import type { Layer1Receipt } from "./action";

export interface Layer1EvidenceRecord {
  recordedAt: string;
  proposalId: string;
  action: TriageDecision["action"];
  receipts: Layer1Receipt[];
}

const DEFAULT_EVIDENCE_PATH = "data/sentinel-layer1-receipts.jsonl";

/** Persist verified Layer 1 evidence as one complete JSON record per cycle. */
export async function appendLayer1Evidence(
  proposalId: string,
  decision: Pick<TriageDecision, "action">,
  receipts: Layer1Receipt[],
  filePath = process.env.SENTINEL_RECEIPT_LOG ?? DEFAULT_EVIDENCE_PATH
): Promise<Layer1EvidenceRecord> {
  if (!proposalId.trim()) {
    throw new Error("Cannot record Layer 1 evidence without a proposal ID.");
  }
  if (receipts.length === 0) {
    throw new Error("Cannot record Layer 1 evidence without verified receipts.");
  }
  for (const receipt of receipts) {
    if (!receipt.signalHash.trim() || !receipt.minerId.trim()) {
      throw new Error("Cannot record Layer 1 evidence with an incomplete receipt.");
    }
  }

  const record: Layer1EvidenceRecord = {
    recordedAt: new Date().toISOString(),
    proposalId,
    action: decision.action,
    receipts,
  };
  await mkdir(dirname(filePath), { recursive: true });
  await appendFile(filePath, `${JSON.stringify(record)}\n`, "utf8");
  return record;
}
