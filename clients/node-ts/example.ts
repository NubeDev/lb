/**
 * roundtrip.ts — login → write a Sample → read it back, against a real
 * `make cloud` node.
 *
 * Run with:
 *   make cloud                          # terminal 1: boot 127.0.0.1:8080
 *   cd clients/node-ts && pnpm install
 *   LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme pnpm example
 *   # or with an API key:
 *   LB_KEY=lbk_acme.k7f3a.ABCDEF23 pnpm example
 */

import { Client, writeSamples, latestSample, callMcp } from "./src/index.js";

const url = process.env.LB_URL ?? "http://127.0.0.1:8080";
const key = process.env.LB_KEY;
const user = process.env.LB_USER ?? "ada";
const ws = process.env.LB_WORKSPACE ?? "acme";

async function main(): Promise<void> {
  let client = new Client(url, "placeholder");
  if (key) {
    client = client.withBearer(key);
  } else {
    const { reply } = await client.login(user, ws);
    console.log(`logged in as ${reply.principal} in ${reply.workspace}`);
  }

  // 1. Push one Sample. `producer` is host-forced to the principal, so omit it.
  const now = Date.now();
  const written = await writeSamples(client, [
    {
      series: "demo.cpu_temp",
      ts: now,
      seq: 1,
      payload: 61.4,
      labels: { host: "pi-7" },
    },
  ]);
  console.log(`accepted=${written.accepted} committed=${written.committed}`);

  // 2. Read the newest value back — the round-trip.
  const latest = await latestSample(client, "demo.cpu_temp");
  console.log("latest sample:", JSON.stringify(latest, null, 2));

  // 3. The universal MCP bridge: every other verb is one call away.
  const seriesList = await callMcp(client, "series.list", {});
  console.log("series in workspace:", seriesList);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
