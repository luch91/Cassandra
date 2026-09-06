import { MIN_MINER_SAMPLE_SIZE } from "./scoring/multi_miner_agreement";
import { discoverCompatibleMiners } from "./scoring/telegraph_client";

export interface SentinelPreflight {
  requiredMinerCount: number;
  compatibleMinerCount: number;
  ready: boolean;
  miners: Array<{ id: string; slug: string; endpoint: string }>;
}

/** Free registry-only readiness check. It never sends inference or payment traffic. */
export async function runSentinelPreflight(): Promise<SentinelPreflight> {
  const miners = await discoverCompatibleMiners("FRAUD_DETECTION");
  return {
    requiredMinerCount: MIN_MINER_SAMPLE_SIZE,
    compatibleMinerCount: miners.length,
    ready: miners.length >= MIN_MINER_SAMPLE_SIZE,
    miners: miners.map((miner) => ({ id: miner.id, slug: miner.slug, endpoint: miner.endpoint.path })),
  };
}
