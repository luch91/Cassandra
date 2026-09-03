import { discoverMiners } from "./scoring/telegraph_client.ts";

async function main(): Promise<void> {
  const miners = await discoverMiners("FRAUD_DETECTION");
  const compatible = miners.filter((m) =>
    (m.activation_status === "active" || m.status === "active") &&
    (m.supported_intents ?? m.intents ?? []).includes("FRAUD_DETECTION")
  );
  console.log(JSON.stringify({
    intent: "FRAUD_DETECTION",
    active_compatible_miners: compatible.length,
    miners: compatible.map(({ id, name, min_price_usdc, status }) => ({ id, name, min_price_usdc, status })),
    pass: compatible.length >= 3,
  }, null, 2));
  if (compatible.length < 3) process.exitCode = 1;
}

main().catch((error: Error) => { console.error(error.message); process.exitCode = 1; });
