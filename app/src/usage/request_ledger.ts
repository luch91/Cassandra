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

export interface UsageBudget {
  maxCompletedRequests: number;
  maxBudgetUsd: number;
  maxRequestCostUsd: number;
}

export class UsageBudgetReachedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "UsageBudgetReachedError";
  }
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

/** Fails closed before a paid call when the configured hard stop is reached. */
export function assertCanStartPaidRequest(metrics: UsageMetrics, budget: UsageBudget): void {
  if (metrics.completedRequests >= budget.maxCompletedRequests) {
    throw new UsageBudgetReachedError(
      `Sentinel request limit reached: ${metrics.completedRequests}/${budget.maxCompletedRequests} completed requests.`,
    );
  }
  if (metrics.totalCostUsd + budget.maxRequestCostUsd > budget.maxBudgetUsd + 1e-9) {
    throw new UsageBudgetReachedError(
      `Sentinel budget would be exceeded: $${metrics.totalCostUsd.toFixed(6)} spent plus ` +
        `$${budget.maxRequestCostUsd.toFixed(6)} maximum request cost exceeds $${budget.maxBudgetUsd.toFixed(6)}.`,
    );
  }
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
