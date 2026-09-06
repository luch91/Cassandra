import { runSentinelPreflight } from "./preflight";

void runSentinelPreflight().then((result) => {
  console.log(JSON.stringify(result, null, 2));
});
