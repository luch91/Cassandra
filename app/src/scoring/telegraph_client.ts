/**
 * Telegraph Engine client for Sentinel.
 *
 * The public integration registry is the source of truth for active miners,
 * their endpoints, and their input schemas. Sentinel only selects miners that
 * explicitly accept a plain-language `query` without additional required
 * fields, so governance-proposal requests are never sent to an incompatible
 * miner.
 */

import { wrapFetchWithPaymentFromConfig } from "@x402/fetch";
import { ExactEvmScheme } from "@x402/evm";

export type TelegraphIntentId = "FRAUD_DETECTION" | "CONTENT_VERIFICATION" | "AI_TEXT_DETECTION";
export type TelegraphHttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export interface MinerEndpoint {
  path: string;
  method: TelegraphHttpMethod;
  description?: string;
}

export interface MinerIntegration {
  id: string;
  slug: string;
  name: string;
  activation_status: string;
  supported_intents: string[];
  endpoints: MinerEndpoint[];
  input_schema?: {
    properties?: Record<string, unknown>;
    required?: string[];
  } | null;
}

export interface CompatibleMiner {
  id: string;
  slug: string;
  name: string;
  endpoint: MinerEndpoint;
}

export interface AskResult {
  miner_id: string;
  miner_name: string;
  result: unknown;
  cost_usd: number;
  duration_ms: number;
  signal_hash?: string;
}

export interface TelegraphSignalVerification {
  signal_hash: string;
  verification: {
    verified: true;
    algorithm: string;
    commitment: string;
  };
  result: {
    status: "success";
  };
  [key: string]: unknown;
}

export type FetchLike = typeof fetch;

const TELEGRAPH_NODE_URL = process.env.TELEGRAPH_NODE_URL ?? "http://13.237.89.59:7044";

function engineUrl(path: string): string {
  return `${TELEGRAPH_NODE_URL}/engine${path}`;
}

/** Prevents a configured wallet from spending unless an operator opts in per run. */
export function assertPaidRequestsEnabled(env: NodeJS.ProcessEnv = process.env): void {
  if (env.SENTINEL_ALLOW_PAID_REQUESTS !== "true") {
    throw new Error("Paid Telegraph requests are disabled. Set SENTINEL_ALLOW_PAID_REQUESTS=true to authorize them.");
  }
}

async function getFetchWithPayment() {
  assertPaidRequestsEnabled();
  const privateKey = process.env.EVM_PRIVATE_KEY;
  if (!privateKey) {
    throw new Error("EVM_PRIVATE_KEY is not set. Sentinel needs a funded Base Sepolia wallet for x402 requests.");
  }
  if (!privateKey.startsWith("0x")) {
    throw new Error("EVM_PRIVATE_KEY must be a 0x-prefixed hexadecimal private key.");
  }

  const { privateKeyToAccount } = await import("viem/accounts");
  const account = privateKeyToAccount(privateKey as `0x${string}`);
  return wrapFetchWithPaymentFromConfig(fetch, {
    schemes: [{ network: "eip155:84532", client: new ExactEvmScheme(account) }],
  });
}

export function selectCompatibleMiners(
  integrations: MinerIntegration[],
  intent: TelegraphIntentId
): CompatibleMiner[] {
  return integrations.flatMap((miner) => {
    const acceptsQuery = Boolean(miner.input_schema?.properties?.query);
    const requiredFields = miner.input_schema?.required ?? [];
    if (
      miner.activation_status !== "active" ||
      !miner.supported_intents.includes(intent) ||
      !acceptsQuery ||
      requiredFields.some((field) => field !== "query")
    ) {
      return [];
    }

    const endpoint = miner.endpoints.find((candidate) => {
      const description = candidate.description?.toUpperCase() ?? "";
      return (
        candidate.method === "POST" &&
        description.includes(intent) &&
        !description.includes("NOT AN INTENT TARGET") &&
        !description.includes("DO NOT ROUTE")
      );
    });
    return endpoint ? [{ id: miner.id, slug: miner.slug, name: miner.name, endpoint }] : [];
  });
}

/** Free public registry discovery. It makes no inference or payment request. */
export async function discoverCompatibleMiners(
  intent: TelegraphIntentId,
  fetcher: FetchLike = fetch
): Promise<CompatibleMiner[]> {
  const res = await fetcher(`${TELEGRAPH_NODE_URL}/miner-dispatcher/integrations`);
  if (!res.ok) {
    throw new Error(`Failed to discover Telegraph integrations: HTTP ${res.status}`);
  }
  const integrations = (await res.json()) as MinerIntegration[];
  return selectCompatibleMiners(integrations, intent);
}

function parseAskResult(miner: CompatibleMiner, payload: unknown): AskResult {
  if (!payload || typeof payload !== "object") {
    throw new Error(`Miner ${miner.slug} returned a non-object response.`);
  }
  const response = payload as Record<string, unknown>;
  const cost = response.cost_usd;
  const duration = response.duration_ms;
  return {
    miner_id: typeof response.miner_id === "string" ? response.miner_id : miner.id,
    miner_name: typeof response.miner_name === "string" ? response.miner_name : miner.name,
    result: response.result ?? response,
    cost_usd: typeof cost === "number" ? cost : 0,
    duration_ms: typeof duration === "number" ? duration : 0,
    signal_hash: typeof response.signal_hash === "string" ? response.signal_hash : undefined,
  };
}

/** Pays for a declared direct endpoint. Production code only calls Telegraph. */
export async function askMiner(miner: CompatibleMiner, query: string): Promise<AskResult> {
  const fetchWithPayment = await getFetchWithPayment();
  const res = await fetchWithPayment(engineUrl(`/v1/ask/${encodeURIComponent(miner.slug)}`), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method: miner.endpoint.method, endpoint: miner.endpoint.path, payload: { query } }),
  });
  if (!res.ok) {
    throw new Error(`Ask request to miner ${miner.slug} failed: HTTP ${res.status}`);
  }
  return parseAskResult(miner, await res.json());
}

/**
 * Requests distinct, schema-compatible live miners in sequence. The optional
 * callback records each completed paid request before a later call can fail.
 */
export async function askMultipleMiners(
  intent: TelegraphIntentId,
  query: string,
  sampleSize: number,
  onResult?: (result: AskResult) => Promise<unknown>
): Promise<AskResult[]> {
  const miners = await discoverCompatibleMiners(intent);
  if (miners.length < sampleSize) {
    throw new Error(`Only ${miners.length} compatible active miners are available for ${intent}, need ${sampleSize}.`);
  }
  const results: AskResult[] = [];
  for (const miner of miners.slice(0, sampleSize)) {
    const result = await askMiner(miner, query);
    await onResult?.(result);
    results.push(result);
  }
  return results;
}

/**
 * Verifies a Telegraph Layer 1 receipt. A 2xx response alone is insufficient:
 * the returned hash, execution status, and cryptographic verification marker
 * must all confirm the supplied receipt.
 */
export async function verifySignal(
  signalHash: string,
  fetcher: FetchLike = fetch
): Promise<TelegraphSignalVerification> {
  if (!signalHash.trim()) {
    throw new Error("Cannot verify an empty signal hash.");
  }
  const res = await fetcher(engineUrl(`/v1/signal/${encodeURIComponent(signalHash)}`));
  if (!res.ok) {
    throw new Error(`Failed to verify signal ${signalHash}: HTTP ${res.status}`);
  }
  const payload = (await res.json()) as unknown;
  if (!payload || typeof payload !== "object") {
    throw new Error(`Signal verification for ${signalHash} returned a non-object response.`);
  }

  const verification = payload as Record<string, unknown>;
  if (verification.signal_hash !== signalHash) {
    throw new Error(`Signal verification returned a different signal hash for ${signalHash}.`);
  }
  const result = verification.result;
  if (!result || typeof result !== "object" || (result as Record<string, unknown>).status !== "success") {
    throw new Error(`Signal verification did not confirm successful execution for ${signalHash}.`);
  }
  const proof = verification.verification;
  if (!proof || typeof proof !== "object" || (proof as Record<string, unknown>).verified !== true) {
    throw new Error(`Signal verification did not cryptographically verify ${signalHash}.`);
  }

  return payload as TelegraphSignalVerification;
}
