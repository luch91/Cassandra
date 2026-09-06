import { getUsageMetrics } from "./usage/request_ledger";

void getUsageMetrics().then((metrics) => {
  console.log(JSON.stringify(metrics, null, 2));
});
