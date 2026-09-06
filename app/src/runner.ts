import { loadSentinelConfig } from "./config";
import { runSentinelCycle } from "./index";

export function startSentinel(): () => void {
  const config = loadSentinelConfig();
  let running = false;
  let timer: NodeJS.Timeout | undefined;

  const run = async () => {
    if (running) return;
    running = true;
    try {
      await runSentinelCycle(config);
    } catch (error) {
      console.error("Sentinel cycle failed. No automatic retry is performed.", error);
    } finally {
      running = false;
      timer = setTimeout(run, config.pollIntervalMs);
    }
  };

  void run();
  return () => {
    if (timer) clearTimeout(timer);
  };
}
