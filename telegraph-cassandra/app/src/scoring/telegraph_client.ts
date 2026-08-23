/**
 * Real Telegraph Engine client for Sentinel, using the confirmed x402
 * payment flow. Source: docs.telegraphprotocol.com/docs/using/x402-inference
 *
 * Flow: discover live miners for an intent -> POST an ask request -> get
 * an HTTP 402 challenge back -> sign a USDC payment -> retry with the
 * payment header -> receive the miner's answer plus a signal_hash receipt.
 *
 * This is Layer 1 of Sentinel's "on-chain action" (see PROJECT_SPEC.md
 * Section 5.1, decision D10): the payment itself is the on-chain-settled
 * receipt, verifiable independently via GET /engine/v1/signal/{signal_hash}.
 * Layer 2 (a DAO-specific "flag this proposal" write) is a separate
 * concern, see onchain/action.ts.
 */

import { wrapFetchWithPayment } from "@x402/fetch";
import { createSigner } from "@x402/evm";

export type TelegraphIntentId =
  | "FRAUD_DETECTION"
  | "CONTENT_VERIFICATION"
  | "AI_TEXT_DETECTION"
  | "AGENT_TASK";

export interface MinerCatalogEntry {
  id: string;
  name: string;
  intents: TelegraphIntentId[];
  min_price_usdc: number;
  status: string;
}

export interface AskResult {
  miner_id: string;
  miner_name: string;
  result: unknown; // shape depends on the specific miner's declared output schema
  cost_usd: number;
  duration_ms: number;
  signal_hash: string;
}

const TELEGRAPH_NODE_URL = process.env.TELEGRAPH_NODE_URL ?? "https://devnode.telegraphprotocol.com";

function getFetchWithPayment() {
  const privateKey = process.env.EVM_PRIVATE_KEY;
  if (!privateKey) {
    throw new Error(
      "EVM_PRIVATE_KEY is not set. Sentinel needs a funded testnet wallet " +
      "(USDC on Base Sepolia) to pay for x402 requests. See .env.example."
    );
  }
  const signer = createSigner(privateKey);
  return wrapFetchWithPayment(fetch, signer);
}

/**
 * GET /api/miners?intent=... Discovery endpoint, no payment required.
 * Always call this fresh rather than caching a hardcoded miner list, per
 * the docs: "the set of live miners changes as operators register and
 * deregister on-chain, treat this endpoint as the source of truth."
 */
export async function discoverMiners(intent: TelegraphIntentId): Promise<MinerCatalogEntry[]> {
  const res = await fetch(`${TELEGRAPH_NODE_URL}/api/miners?intent=${intent}&status=active`);
  if (!res.ok) {
    throw new Error(`Failed to discover miners for ${intent}: HTTP ${res.status}`);
  }
  return res.json();
}

/**
 * Pays for and executes a single ask request against one miner. Handles
 * the full 402 challenge/payment/retry cycle via @x402/fetch.
 *
 * Non-negotiable: this must only ever call the real Telegraph endpoint.
 * Never fabricate a response here, if the call fails, let it throw.
 */
export async function askMiner(minerId: string, query: string): Promise<AskResult> {
  const fetchWithPayment = getFetchWithPayment();
  const res = await fetchWithPayment(`${TELEGRAPH_NODE_URL}/engine/v1/ask/${minerId}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ query }),
  });

  if (!res.ok) {
    throw new Error(`Ask request to miner ${minerId} failed: HTTP ${res.status}`);
  }

  return res.json();
}

/**
 * Independently verifies a call after the fact via its signal_hash. This
 * is how Layer 1's "on-chain receipt" gets checked, not trusting the
 * original response alone.
 */
export async function verifySignal(signalHash: string): Promise<unknown> {
  const res = await fetch(`${TELEGRAPH_NODE_URL}/engine/v1/signal/${signalHash}`);
  if (!res.ok) {
    throw new Error(`Failed to verify signal ${signalHash}: HTTP ${res.status}`);
  }
  return res.json();
}

/**
 * Queries N distinct live miners for the same intent and query, used by
 * multi_miner_agreement.ts to compute an app-layer confidence signal.
 * See that file's header comment for why this replaces calling DWCS
 * directly, which isn't possible from application code.
 */
export async function askMultipleMiners(
  intent: TelegraphIntentId,
  query: string,
  sampleSize: number
): Promise<AskResult[]> {
  const miners = await discoverMiners(intent);
  if (miners.length < sampleSize) {
    throw new Error(
      `Only ${miners.length} live miner(s) available for ${intent}, need at least ${sampleSize}. ` +
      `This also means the intent likely doesn't clear the 3-active-Miner guardrail yet.`
    );
  }
  const chosen = miners.slice(0, sampleSize);
  return Promise.all(chosen.map((m) => askMiner(m.id, query)));
}
