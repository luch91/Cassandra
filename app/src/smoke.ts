import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import {
  askMiner,
  assertPaidRequestsEnabled,
  discoverCompatibleMiners,
  type AskResult,
  type CompatibleMiner,
  type TelegraphSignalVerification,
} from "./scoring/telegraph_client";
import { verifyLayer1Receipts, type Layer1Receipt } from "./onchain/action";

export interface SentinelSmokeEvidence {
  recordedAt: string;
  minerId: string;
  minerSlug: string;
  endpoint: string;
  signalHash: string;
  costUsd: number;
  durationMs: number;
  verifiedAt: string;
  verification: { algorithm: string; commitment: string };
}

export interface SmokeDependencies {
  discover: (intent: "FRAUD_DETECTION") => Promise<CompatibleMiner[]>;
  ask: (miner: CompatibleMiner, query: string) => Promise<AskResult>;
  verify: (results: AskResult[]) => Promise<Layer1Receipt[]>;
}

const DEFAULT_EVIDENCE_PATH = ".sentinel-evidence/layer1-smoke.json";

/** Selects only the two reviewed proposal-semantic routes, in priority order. */
export function selectProposalSmokeMiner(miners: CompatibleMiner[], query: string): CompatibleMiner {
  const isProposalRoute = (miner: CompatibleMiner): boolean =>
    miner.endpoint.method === "POST" &&
    (miner.endpoint.description?.toUpperCase().includes("FRAUD_DETECTION") ?? false);
  const sarzOps = miners.find((miner) =>
    isProposalRoute(miner) && (miner.id === "91001" || miner.slug === "sarzops-transaction-risk") &&
    miner.endpoint.path === "/fraud"
  );
  if (sarzOps) return sarzOps;

  const queryLength = [...query].length;
  const txLens = miners.find((miner) =>
    isProposalRoute(miner) && (miner.id === "9002" || miner.slug === "txlens") &&
    miner.endpoint.path === "/fraud-query"
  );
  if (txLens && queryLength <= 1000) return txLens;

  throw new Error(
    txLens
      ? "No proposal-semantic smoke route is available for this query length."
      : "No reviewed proposal-semantic FRAUD_DETECTION route is available; refusing to spend."
  );
}

/** Runs exactly one explicitly authorized request. It is not a traffic generator or loop. */
export async function runSentinelSmoke(
  query: string,
  env: NodeJS.ProcessEnv = process.env,
  dependencies: SmokeDependencies = { discover: discoverCompatibleMiners, ask: askMiner, verify: verifyLayer1Receipts },
  evidencePath = env.SENTINEL_SMOKE_EVIDENCE ?? DEFAULT_EVIDENCE_PATH
): Promise<SentinelSmokeEvidence> {
  if (!query.trim()) throw new Error("SENTINEL_SMOKE_QUERY must be a non-empty query.");
  assertPaidRequestsEnabled(env);
  const miners = await dependencies.discover("FRAUD_DETECTION");
  if (miners.length === 0) throw new Error("No compatible active FRAUD_DETECTION miner is available for the smoke test.");

  // Deliberately select one reviewed semantic route and make one request. Do not add retries.
  const miner = selectProposalSmokeMiner(miners, query);
  const result = await dependencies.ask(miner, query);
  const [receipt] = await dependencies.verify([result]);
  const verification = receipt.verification as TelegraphSignalVerification;
  const record: SentinelSmokeEvidence = {
    recordedAt: new Date().toISOString(),
    minerId: result.miner_id,
    minerSlug: miner.slug,
    endpoint: `${miner.endpoint.method} ${miner.endpoint.path}`,
    signalHash: receipt.signalHash,
    costUsd: result.cost_usd,
    durationMs: result.duration_ms,
    verifiedAt: receipt.verifiedAt,
    verification: { algorithm: verification.verification.algorithm, commitment: verification.verification.commitment },
  };
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(evidencePath, `${JSON.stringify(record, null, 2)}\n`, "utf8");
  return record;
}
