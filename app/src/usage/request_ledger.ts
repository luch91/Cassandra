import { appendFile, mkdir, readFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { AskResult, TelegraphIntentId } from "../scoring/telegraph_client";

export interface RequestLedgerRecord {
  recordedAt: string;
  proposalId: string;
  intent: TelegraphIntentId;
  minerId: string;
  minerName: string;
  costUsd: number;
  durationMs: number;
  signalHash?: string;
}

export interface UsageMetrics {
  completedRequests: number;
  uniqueProposals: number;
  totalCostUsd: number;
  targetRequests: number;
  targetReached: boolean;
}

const DEFAULT_LEDGER_PATH = "data/sentinel-request-ledger.jsonl";

export async function readRequestLedger(filePath = process.env.SENTINEL_REQUEST_LEDGER ?? DEFAULT_LEDGER_PATH): Promise<RequestLedgerRecord[]> {
  try {
    const content = await readFile(filePath, "utf8");
    return content.split("\n").filter(Boolean).map((line) => JSON.parse(line) as RequestLedgerRecord);
  } catch (error: unknown) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }
}

export async function readProcessedProposalIds(filePath?: string): Promise<Set<string>> {
  return new Set((await readRequestLedger(filePath)).map((record) => record.proposalId));
}

/** Counts only completed, attributable records. It never generates traffic. */
export async function getUsageMetrics(
  filePath?: string,
  targetRequests = 100
): Promise<UsageMetrics> {
  if (!Number.isInteger(targetRequests) || targetRequests < 1) {
    throw new Error("targetRequests must be a positive integer.");
  }
  const records = await readRequestLedger(filePath);
  const completedRequests = records.length;
  return {
    completedRequests,
    uniqueProposals: new Set(records.map((record) => record.proposalId)).size,
    totalCostUsd: records.reduce((total, record) => total + record.costUsd, 0),
    targetRequests,
    targetReached: completedRequests >= targetRequests,
  };
}

/** Appends each completed paid request once. This is the non-gamed usage ledger. */
export async function appendRequestLedger(
  proposalId: string,
  intent: TelegraphIntentId,
  result: AskResult,
  filePath = process.env.SENTINEL_REQUEST_LEDGER ?? DEFAULT_LEDGER_PATH
): Promise<RequestLedgerRecord> {
  if (!proposalId.trim() || !result.miner_id.trim()) {
    throw new Error("Cannot record a request without a proposal ID and miner ID.");
  }
  const record: RequestLedgerRecord = {
    recordedAt: new Date().toISOString(),
    proposalId,
    intent,
    minerId: result.miner_id,
    minerName: result.miner_name,
    costUsd: result.cost_usd,
    durationMs: result.duration_ms,
    signalHash: result.signal_hash,
  };
  await mkdir(dirname(filePath), { recursive: true });
  await appendFile(filePath, `${JSON.stringify(record)}\n`, "utf8");
  return record;
}
