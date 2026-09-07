import { loadSentinelConfig } from "./config";
import { runSentinelCycle } from "./index";
import { UsageBudgetReachedError } from "./usage/request_ledger";

export function startSentinel(): () => void {
  const config = loadSentinelConfig();
  let running = false;
  let stopped = false;
  let timer: NodeJS.Timeout | undefined;

  const run = async () => {
    if (running) return;
    running = true;
    try {
      await runSentinelCycle(config);
    } catch (error) {
      if (error instanceof UsageBudgetReachedError) {
        stopped = true;
        console.log(`Sentinel stopped: ${error.message}`);
      } else {
        console.error("Sentinel cycle failed. No automatic retry is performed.", error);
      }
    } finally {
      running = false;
      if (!stopped) timer = setTimeout(run, config.pollIntervalMs);
    }
  };

  void run();
  return () => {
    if (timer) clearTimeout(timer);
  };
}
