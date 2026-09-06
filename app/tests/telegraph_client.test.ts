import { selectCompatibleMiners } from "../src/scoring/telegraph_client";

describe("selectCompatibleMiners", () => {
  const integrations = [
    {
      id: "1",
      slug: "query-miner",
      name: "Query Miner",
      activation_status: "active",
      supported_intents: ["FRAUD_DETECTION"],
      input_schema: { properties: { query: { type: "string" } }, required: ["query"] },
      endpoints: [{ path: "/scan", method: "POST" as const, description: "FRAUD_DETECTION analysis" }],
    },
    {
      id: "2",
      slug: "needs-address",
      name: "Address Miner",
      activation_status: "active",
      supported_intents: ["FRAUD_DETECTION"],
      input_schema: { properties: { query: {}, address: {} }, required: ["query", "address"] },
      endpoints: [{ path: "/scan", method: "POST" as const, description: "FRAUD_DETECTION analysis" }],
    },
    {
      id: "3",
      slug: "inactive",
      name: "Inactive Miner",
      activation_status: "inactive",
      supported_intents: ["FRAUD_DETECTION"],
      input_schema: { properties: { query: {} } },
      endpoints: [{ path: "/scan", method: "POST" as const, description: "FRAUD_DETECTION analysis" }],
    },
    {
      id: "4",
      slug: "product-endpoint-first",
      name: "Product Endpoint First",
      activation_status: "active",
      supported_intents: ["FRAUD_DETECTION"],
      input_schema: { properties: { query: {} } },
      endpoints: [
        { path: "/product", method: "POST" as const, description: "NOT AN INTENT TARGET. Do not route FRAUD_DETECTION here." },
        { path: "/scan", method: "POST" as const, description: "FRAUD_DETECTION analysis" },
      ],
    },
  ];

  it("selects only active miners with a declared query-compatible endpoint", () => {
    expect(selectCompatibleMiners(integrations, "FRAUD_DETECTION")).toEqual([
      { id: "1", slug: "query-miner", name: "Query Miner", endpoint: integrations[0].endpoints[0] },
      { id: "4", slug: "product-endpoint-first", name: "Product Endpoint First", endpoint: integrations[3].endpoints[1] },
    ]);
  });
});
