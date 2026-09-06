import { appendFile, mkdir, readFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { TelegraphIntentId } from "../scoring/telegraph_client";

interface ProposalAttemptRecord {
  recordedAt: string;
  proposalId: string;
  intent: TelegraphIntentId;
}

const DEFAULT_ATTEMPT_PATH = "data/sentinel-proposal-attempts.jsonl";

export async function readAttemptedProposalIds(filePath = process.env.SENTINEL_ATTEMPT_LEDGER ?? DEFAULT_ATTEMPT_PATH): Promise<Set<string>> {
  try {
    const content = await readFile(filePath, "utf8");
    return new Set(
      content.split("\n").filter(Boolean).map((line) => (JSON.parse(line) as ProposalAttemptRecord).proposalId)
    );
  } catch (error: unknown) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return new Set();
    throw error;
  }
}

/** Marks a proposal before inference so polling can never replay it automatically. */
export async function appendProposalAttempt(
  proposalId: string,
  intent: TelegraphIntentId,
  filePath = process.env.SENTINEL_ATTEMPT_LEDGER ?? DEFAULT_ATTEMPT_PATH
): Promise<void> {
  if (!proposalId.trim()) {
    throw new Error("Cannot record a proposal attempt without a proposal ID.");
  }
  await mkdir(dirname(filePath), { recursive: true });
  const record: ProposalAttemptRecord = { recordedAt: new Date().toISOString(), proposalId, intent };
  await appendFile(filePath, `${JSON.stringify(record)}\n`, "utf8");
}
