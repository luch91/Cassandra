import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { discoverMiners } from "./scoring/telegraph_client.ts";
import type { MinerCatalogEntry } from "./scoring/telegraph_client.ts";

const EVIDENCE_DIR = process.env.SENTINEL_EVIDENCE_DIR ?? ".sentinel-evidence";
const LEDGER_PATH = `${EVIDENCE_DIR}/paid-smoke-ledger.json`;
const SENSITIVE_HEADERS = /authorization|payment|signature|cookie|secret|token|api[-_]?key/i;

export function assertPaidSmokeOptIn(env: NodeJS.ProcessEnv, args: string[]): void {
  if (env.SENTINEL_ALLOW_PAID_REQUESTS !== "true") {
    throw new Error("Refusing paid smoke test: SENTINEL_ALLOW_PAID_REQUESTS=true is required.");
  }
  if (!args.includes("--confirm-paid-smoke")) {
    throw new Error("Refusing paid smoke test: --confirm-paid-smoke is required.");
  }
}

function arg(args: string[], name: string): string {
  const i = args.indexOf(name);
  if (i < 0 || !args[i + 1]) throw new Error(`Missing required argument ${name} <value>.`);
  return args[i + 1];
}

function redactHeaders(headers: Headers): Record<string, string> {
  const out: Record<string, string> = {};
  headers.forEach((value, key) => {
    out[key] = SENSITIVE_HEADERS.test(key) ? "[REDACTED]" : value;
  });
  return out;
}

async function readLedger(): Promise<Record<string, unknown>> {
  try { return JSON.parse(await readFile(LEDGER_PATH, "utf8")); }
  catch { return {}; }
}

export async function runPaidSmoke(args: string[]): Promise<void> {
  assertPaidSmokeOptIn(process.env, args);
  const minerId = arg(args, "--miner-id");
  const query = arg(args, "--query");

  const miners = await discoverMiners("FRAUD_DETECTION");
  const compatible: MinerCatalogEntry[] = miners.filter((m) =>
    (m.activation_status === "active" || m.status === "active") &&
    (m.supported_intents ?? m.intents ?? []).includes("FRAUD_DETECTION")
  );
  if (compatible.length < 3) {
    throw new Error(`Preflight blocked: found ${compatible.length} compatible active FRAUD_DETECTION miners; need at least 3.`);
  }
  const selected = compatible.find((m) => m.id === minerId);
  if (!selected) throw new Error(`Preflight blocked: ${minerId} is not a declared compatible active miner.`);
  const endpoint = selected.endpoints?.find((e) =>
    e.description?.includes("FRAUD_DETECTION") || e.path.toLowerCase().includes("fraud")
  );
  if (!endpoint) throw new Error(`Preflight blocked: ${minerId} has no declared FRAUD_DETECTION endpoint.`);

  const ledger = await readLedger();
  const requestKey = createHash("sha256").update(`${minerId}\n${query}`).digest("hex");
  if ((ledger[requestKey] as { paid?: boolean } | undefined)?.paid) throw new Error(`Refusing repeat paid smoke request: ${requestKey}.`);

  const { wrapFetchWithPaymentFromConfig } = await import("@x402/fetch");
  const { ExactEvmScheme } = await import("@x402/evm");
  const { privateKeyToAccount } = await import("viem/accounts");
  const key = process.env.EVM_PRIVATE_KEY;
  if (!key?.startsWith("0x")) throw new Error("EVM_PRIVATE_KEY is required and must be 0x-prefixed.");
  const account = privateKeyToAccount(key as `0x${string}`);
  const paidFetch = wrapFetchWithPaymentFromConfig(fetch, {
    schemes: [{ network: "eip155:84532", client: new ExactEvmScheme(account) }],
  });

  const startedAt = new Date().toISOString();
  const response = await paidFetch(`${process.env.TELEGRAPH_NODE_URL ?? "https://devnode.telegraphprotocol.com"}/engine/v1/ask/${minerId}`, {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method: endpoint.method, endpoint: endpoint.path, payload: { query } }),
  });
  const body = await response.text();
  const evidence = {
    started_at: startedAt,
    completed_at: new Date().toISOString(),
    request_key: requestKey,
    miner_id: minerId,
    miner_name: selected.name,
    miner_endpoint: endpoint,
    intent: "FRAUD_DETECTION",
    status: response.status,
    headers: redactHeaders(response.headers),
    body,
    note: "Exactly one paid request was authorized by the one-shot command.",
  };
  await mkdir(EVIDENCE_DIR, { recursive: true });
  await writeFile(`${EVIDENCE_DIR}/${requestKey}.json`, JSON.stringify(evidence, null, 2));
  ledger[requestKey] = { miner_id: minerId, created_at: startedAt, evidence: `${requestKey}.json`, paid: response.ok };
  await writeFile(LEDGER_PATH, JSON.stringify(ledger, null, 2));
  if (!response.ok) throw new Error(`Paid smoke request failed: HTTP ${response.status}; evidence saved.`);
  console.log(JSON.stringify({ status: response.status, miner_id: minerId, evidence: `${EVIDENCE_DIR}/${requestKey}.json`, request_key: requestKey }));
}

if (process.argv[1]?.endsWith("app/src/smoke.ts")) {
  runPaidSmoke(process.argv.slice(2)).catch((error: Error) => { console.error(error.message); process.exitCode = 1; });
}
