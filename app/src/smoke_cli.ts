import { runSentinelSmoke } from "./smoke";

const query = process.env.SENTINEL_SMOKE_QUERY;
if (!query) {
  console.error("Set SENTINEL_SMOKE_QUERY and SENTINEL_ALLOW_PAID_REQUESTS=true to run the one-shot smoke test.");
  process.exitCode = 1;
} else {
  void runSentinelSmoke(query).then((evidence) => console.log(JSON.stringify(evidence, null, 2))).catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
